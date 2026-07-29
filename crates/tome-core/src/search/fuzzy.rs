//! Typo tolerance by term correction (S2-5, spec P2-009).
//!
//! # Why this does not use `FuzzyTermQuery`
//!
//! The obvious implementation, and the one P2-009's technical note sketches,
//! is tantivy's [`FuzzyTermQuery`]. It is built on an
//! `AutomatonWeight`, and an `AutomatonWeight` produces a **`ConstScorer`** —
//! every document it matches scores exactly the same. Dropping one into the
//! query would therefore hand every fuzzy hit an identical score and throw
//! away BM25 for that term: the page whose *subject* is environment variables
//! would rank level with one that mentions them once in a footnote, and none
//! of S2-4's tuning would reach it.
//!
//! [`FuzzyTermQuery`]: tantivy::query::FuzzyTermQuery
//!
//! So this corrects the **query** instead of relaxing the match. A term that
//! appears nowhere in the index is looked up in the term dictionary, the best
//! correction is found, and the correction is searched as an ordinary term —
//! scored by BM25, boosted by field, penalised by length, exactly like
//! anything else the user typed. Three things fall out of that choice:
//!
//! - **"Did you mean?" is free.** P2-009 asks for it, and a query relaxation
//!   cannot answer it — `FuzzyTermQuery` never reveals which terms it matched.
//!   Here the correction *is* the mechanism, so [`Suggestion`] is a by-product.
//! - **"Fuzzy ranks strictly below exact" is structural, not enforced.**
//!   Corrections are only generated for terms that match *nothing*, so no
//!   document exists that matched the typo exactly and could be displaced.
//! - **The correction is visible in the score.** A wrong correction is a
//!   ranking bug that shows up in the eval set, rather than a flat bonus
//!   smeared over hundreds of documents where nothing stands out.
//!
//! # The limitation, stated plainly
//!
//! Candidates are found by scanning the term dictionary from a **prefix of the
//! first [`PREFIX_CHARS`] characters**, so a typo *inside* that prefix is not
//! corrected: `teh` will not find `the`. A Levenshtein automaton would have no
//! such blind spot, and `levenshtein-automata` plus `tantivy-fst` would have to
//! become direct dependencies, pinned to whatever versions tantivy resolves, to
//! get one — `DfaWrapper` is `pub(crate)`.
//!
//! The prefix is why this is affordable: without it, correcting one term means
//! reading every term in the dictionary, and SPIKE-003's 100 000-page index has
//! a large one. Terms shorter than four characters are never corrected anyway
//! (P2-009's distance schedule gives them distance 0), so every correctable
//! term has a prefix to scan from.

use std::collections::HashMap;

use tantivy::schema::Field;
use tantivy::{Searcher, Term};

use crate::error::{Error, Result};

/// How many leading characters a correction must share with what was typed.
///
/// Three is a deliberate trade. Longer is faster and blinder; shorter reads
/// more of the dictionary for each correction. Typing errors overwhelmingly
/// preserve the opening of a word — the corpus's own misspellings
/// (`enviroment`, `manifset`, `comprehention`, `excepton`, `modual`) all
/// diverge at character four or later — but that is a tendency, not a rule,
/// and it is the reason `teh` does not find `the`.
pub const PREFIX_CHARS: usize = 3;

/// Longest term this will attempt to correct, in characters.
///
/// Edit distance is quadratic in term length, and a "term" this long is a
/// minified blob or a base64 payload rather than a word somebody mistyped.
const MAX_TERM_CHARS: usize = 40;

/// How many dictionary entries one correction may examine before giving up.
///
/// A backstop on latency, not a tuning knob. A short prefix over a large
/// vocabulary can cover a lot of terms, and P2-009 budgets 20 ms for the whole
/// fuzzy path; scanning is the only part that can run away. Hitting this means
/// a *worse* correction, never a wrong one — candidates arrive in dictionary
/// order, so truncation drops arbitrary entries rather than the good ones.
const MAX_CANDIDATES_SCANNED: usize = 50_000;

/// A term the user typed that matched nothing, and what it probably meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The term as typed, lowercased by the tokenizer.
    pub typed: String,
    /// The correction, which is a real term in the index.
    pub meant: String,
}

/// Maximum edit distance permitted for a term of `chars` characters.
///
/// P2-009's schedule: 0 for terms of three characters or fewer, 1 for four to
/// six, 2 beyond. `cap` clamps it further, and **2 is the ceiling** — the
/// original criterion asked for "2-3 for longer words" and 3 is not
/// implementable, because Levenshtein automata top out at 2 and the candidate
/// set explodes long before that matters.
pub(super) fn max_distance(chars: usize, cap: u8) -> u8 {
    let by_length: u8 = match chars {
        0..=3 => 0,
        4..=6 => 1,
        _ => 2,
    };
    by_length.min(cap).min(2)
}

/// Damerau-Levenshtein distance (optimal string alignment), or `None` if it
/// exceeds `max`.
///
/// Transpositions count as one edit rather than two, which is not a detail:
/// `manifset` for `manifest` and `teh` for `the` are transpositions, and they
/// are among the most common typing errors there are. Plain Levenshtein scores
/// them 2 and would need the wider, slower distance to catch what this catches
/// at 1.
///
/// Operates on `char`s throughout. Byte arithmetic here would both mis-measure
/// any non-ASCII term and slice one apart.
pub(super) fn edit_distance_within(typed: &[char], candidate: &[char], max: u8) -> Option<u8> {
    let (n, m) = (typed.len(), candidate.len());
    let max = max as usize;

    // A length difference alone already costs that many insertions.
    if n.abs_diff(m) > max {
        return None;
    }

    // Three rows: the current one, and the two the recurrence reads. Row `k`
    // lives at `k % 3`.
    let mut rows = [
        vec![0usize; m + 1],
        vec![0usize; m + 1],
        vec![0usize; m + 1],
    ];
    for (j, cell) in rows[0].iter_mut().enumerate() {
        *cell = j;
    }

    for i in 1..=n {
        // Taken out so the previous rows can be read while this one is
        // written. `take` leaves an empty vec behind and it is put back at the
        // end of the iteration.
        let mut row = std::mem::take(&mut rows[i % 3]);
        row[0] = i;
        let mut best_in_row = i;

        for j in 1..=m {
            let substitution = usize::from(typed[i - 1] != candidate[j - 1]);
            let mut cost = (rows[(i - 1) % 3][j] + 1)
                .min(row[j - 1] + 1)
                .min(rows[(i - 1) % 3][j - 1] + substitution);

            // Transposition: the previous two characters are swapped.
            if i > 1
                && j > 1
                && typed[i - 1] == candidate[j - 2]
                && typed[i - 2] == candidate[j - 1]
            {
                cost = cost.min(rows[(i - 2) % 3][j - 2] + 1);
            }

            row[j] = cost;
            best_in_row = best_in_row.min(cost);
        }

        // Every later row is at least this good, so the answer cannot come
        // back under `max` once a whole row has exceeded it.
        if best_in_row > max {
            rows[i % 3] = row;
            return None;
        }
        rows[i % 3] = row;
    }

    let distance = rows[n % 3][m];
    if distance > max {
        return None;
    }
    // `distance <= max <= u8::MAX`, so this cannot fail; `try_from` rather
    // than `as` because the workspace forbids silent truncation.
    u8::try_from(distance).ok()
}

/// Find the best correction for each term that appears nowhere in the index.
///
/// `searched` are the fields a query runs against; `dictionaries` are the
/// fields whose vocabularies corrections may be drawn from. They differ on
/// purpose — see [`super::SearchEngine::suggest`].
pub(super) fn corrections(
    searcher: &Searcher,
    terms: &[String],
    searched: &[Field],
    dictionaries: &[Field],
    max_distance_cap: u8,
) -> Result<Vec<Suggestion>> {
    let mut suggestions = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for term in terms {
        if !seen.insert(term.clone()) {
            continue;
        }
        let characters: Vec<char> = term.chars().collect();
        if characters.len() > MAX_TERM_CHARS {
            continue;
        }
        let distance = max_distance(characters.len(), max_distance_cap);
        if distance == 0 {
            continue;
        }
        if is_present(searcher, term, searched)? {
            continue;
        }
        if let Some(correction) = best_correction(searcher, &characters, dictionaries, distance)? {
            suggestions.push(Suggestion {
                typed: term.clone(),
                meant: correction,
            });
        }
    }
    Ok(suggestions)
}

/// Whether a term occurs in any of the searched fields.
fn is_present(searcher: &Searcher, term: &str, fields: &[Field]) -> Result<bool> {
    for field in fields {
        let frequency = searcher
            .doc_freq(&Term::from_field_text(*field, term))
            .map_err(super::index_error)?;
        if frequency > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The nearest real term to `typed`, preferring smaller edit distance and then
/// the more common term.
///
/// Document frequency breaks ties because the commoner word is the likelier
/// intent: `modual` is one edit from both `module` and `modual`-adjacent
/// rarities, and the one that appears in three hundred pages is the one that
/// was meant.
fn best_correction(
    searcher: &Searcher,
    typed: &[char],
    fields: &[Field],
    distance: u8,
) -> Result<Option<String>> {
    let prefix: String = typed.iter().take(PREFIX_CHARS).collect();
    let prefix = prefix.as_bytes();

    // Aggregated across segments: the same term in three segments is one term
    // with the sum of their frequencies, not three candidates.
    let mut candidates: HashMap<String, (u8, u64)> = HashMap::new();
    let mut scanned = 0usize;

    'scan: for segment in searcher.segment_readers() {
        for field in fields {
            let inverted = segment.inverted_index(*field).map_err(super::index_error)?;

            // No upper bound: the dictionary is byte-ordered, so the scan is
            // stopped by the first key that no longer carries the prefix.
            // Computing an upper bound would mean incrementing the last byte
            // of a UTF-8 prefix, which is a needless way to get it wrong.
            let mut stream =
                inverted
                    .terms()
                    .range()
                    .ge(prefix)
                    .into_stream()
                    .map_err(|source| Error::Search {
                        message: source.to_string(),
                    })?;

            while let Some((key, info)) = stream.next() {
                if !key.starts_with(prefix) {
                    break;
                }
                scanned += 1;
                if scanned > MAX_CANDIDATES_SCANNED {
                    tracing::debug!(
                        scanned,
                        "term-correction scan hit its candidate limit; the suggestion may be \
                         worse than the dictionary allows"
                    );
                    break 'scan;
                }
                // Terms are stored as the tokenizer emitted them, so a
                // non-UTF-8 key is not something to correct towards.
                let Ok(candidate) = std::str::from_utf8(key) else {
                    continue;
                };
                let characters: Vec<char> = candidate.chars().collect();
                if characters.len() > MAX_TERM_CHARS {
                    continue;
                }
                let Some(found) = edit_distance_within(typed, &characters, distance) else {
                    continue;
                };
                if found == 0 {
                    // The term is present after all, in a dictionary field
                    // that was not among the searched ones. Unreachable while
                    // `dictionaries` is a subset of `searched`, and a
                    // correction of a word to itself if that ever stops
                    // holding.
                    return Ok(None);
                }
                let entry = candidates.entry(candidate.to_owned()).or_insert((found, 0));
                entry.0 = entry.0.min(found);
                entry.1 += u64::from(info.doc_freq);
            }
        }
    }

    // Closer first, then commoner, then alphabetical so the result does not
    // depend on hash iteration order — an unstable suggestion is worse than a
    // mediocre one, because it makes the eval set unrepeatable.
    Ok(candidates
        .into_iter()
        .min_by(|a, b| {
            (a.1 .0, std::cmp::Reverse(a.1 .1), &a.0).cmp(&(
                b.1 .0,
                std::cmp::Reverse(b.1 .1),
                &b.0,
            ))
        })
        .map(|(term, _)| term))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn distance(a: &str, b: &str, max: u8) -> Option<u8> {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        edit_distance_within(&a, &b, max)
    }

    #[test]
    fn p2_009_distance_schedule() {
        // 0 for <= 3 characters, 1 for 4..=6, 2 beyond, capped at 2.
        assert_eq!(max_distance(1, 2), 0);
        assert_eq!(max_distance(3, 2), 0);
        assert_eq!(max_distance(4, 2), 1);
        assert_eq!(max_distance(6, 2), 1);
        assert_eq!(max_distance(7, 2), 2);
        assert_eq!(max_distance(40, 2), 2);
        // The cap only ever tightens.
        assert_eq!(max_distance(40, 1), 1);
        assert_eq!(max_distance(40, 0), 0);
        // And 2 is the ceiling however high the cap is set: Levenshtein
        // automata do not go further and the candidate set explodes.
        assert_eq!(max_distance(40, 9), 2);
    }

    #[test]
    fn identical_terms_are_distance_zero() {
        assert_eq!(distance("module", "module", 2), Some(0));
        assert_eq!(distance("", "", 2), Some(0));
    }

    #[test]
    fn the_corpus_misspellings_are_reachable_under_p2_009s_schedule() {
        // Real typos from the relevance eval set. If a change here stops
        // reaching them, S2-5's measured gain goes with it.
        for (typed, meant) in [
            ("enviroment", "environment"),
            ("manifset", "manifest"),
            ("comprehention", "comprehension"),
            ("excepton", "exception"),
            ("dependancies", "dependencies"),
            ("quearystring", "querystring"),
            ("dictionarys", "dictionary"),
            ("iterater", "iterator"),
            ("kubernets", "kubernetes"),
            ("sting", "string"),
            ("functoin", "function"),
        ] {
            let limit = max_distance(typed.chars().count(), 2);
            assert!(
                distance(typed, meant, limit).is_some(),
                "{typed:?} -> {meant:?} should be within {limit}"
            );
        }
    }

    #[test]
    fn p2_009s_schedule_cannot_reach_every_real_typo() {
        // Recorded rather than worked around. The schedule is specified — 1
        // edit for a term of 4..=6 characters — and these two need more than
        // it allows, so they stay wrong however good the candidate search is.
        //
        // - `modual` -> `module` is a transposition *and* a substitution, so 2
        //   edits on a 6-character term.
        // - `pth` -> `path` is 1 edit on a 3-character term, and 3 characters
        //   are allowed 0.
        //
        // Both are in the eval corpus and both are expected to fail. Widening
        // the schedule to catch them is a specification change (P2-009), not a
        // tuning decision, and it buys false positives everywhere else:
        // allowing an edit on 3-character terms makes `Vec` match `Vex`,
        // `Vev`, `sec`, `hex`.
        assert_eq!(distance("modual", "module", max_distance(6, 2)), None);
        assert_eq!(max_distance("pth".chars().count(), 2), 0);
        // Both are within reach if the schedule ever widens, which is what
        // makes them a policy choice rather than a capability gap.
        assert_eq!(distance("modual", "module", 2), Some(2));
        assert_eq!(distance("pth", "path", 1), Some(1));
    }

    #[test]
    fn a_transposition_costs_one_edit_not_two() {
        // The reason this is Damerau-Levenshtein. `manifset` is six characters
        // of prefix plus a swap; plain Levenshtein scores it 2, and at six
        // characters P2-009 only allows 1.
        assert_eq!(distance("manifset", "manifest", 1), Some(1));
        assert_eq!(distance("teh", "the", 1), Some(1));
        assert_eq!(distance("functoin", "function", 1), Some(1));
    }

    #[test]
    fn distance_is_symmetric() {
        for (a, b) in [("module", "modual"), ("kitten", "sitting"), ("a", "")] {
            assert_eq!(distance(a, b, 3), distance(b, a, 3), "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn exceeding_the_limit_returns_none_rather_than_a_large_number() {
        assert_eq!(distance("module", "package", 2), None);
        assert_eq!(distance("a", "abcdefgh", 2), None);
        // The classic: three edits.
        assert_eq!(distance("kitten", "sitting", 2), None);
        assert_eq!(distance("kitten", "sitting", 3), Some(3));
    }

    #[test]
    fn multi_byte_terms_are_measured_in_characters() {
        // Byte arithmetic would both mis-measure these and slice one apart.
        assert_eq!(distance("naïve", "naive", 2), Some(1));
        assert_eq!(distance("日本語", "日本", 2), Some(1));
        assert_eq!(distance("café", "cafe", 1), Some(1));
        assert_eq!(distance("🎉a", "🎉b", 1), Some(1));
    }

    #[test]
    fn an_empty_term_is_not_a_correction_target() {
        assert_eq!(distance("", "ab", 2), Some(2));
        assert_eq!(distance("", "abc", 2), None);
    }
}
