//! Query-time ranking parameters (S2-4, spec P2-004).
//!
//! Everything here is applied at *query* time, not at index time, which is the
//! property that makes it tunable: changing any of it needs no reindex, so the
//! sweep in `tests/relevance.rs` can evaluate hundreds of combinations against
//! one index.
//!
//! # What could not be tuned, and why
//!
//! The obvious lever for the defect S2-4 exists to fix — one enormous page
//! outranking specific ones — is BM25's length normalisation, `b`. **It is not
//! reachable.** In tantivy 0.26 `k1` and `b` are private constants in
//! `src/query/bm25.rs`, not weight parameters, so raising `b` would mean
//! forking the crate. [`Ranking::length_pivot`] and
//! [`Ranking::length_penalty`] are the reply to that: a post-hoc divisor
//! applied by the collector, which reaches the same place from outside.
//!
//! This is worth knowing before anyone re-reads the plan and goes looking for
//! `set_bm25_params`. It does not exist, and did not exist when the plan
//! described the lever.

use tantivy::Score;

/// How each field's contribution is weighted, and how length is penalised.
///
/// [`Ranking::TUNED`] holds the measured values; [`Default`] returns it.
/// The struct is what the eval sweep varies, so nothing here may become a
/// constant again without taking that sweep away.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ranking {
    /// Field boosts. Applied by the query parser, so they scale the whole of
    /// each field's BM25 contribution.
    pub title: f32,
    pub headers: f32,
    pub body: f32,
    pub code: f32,

    /// Body length, in tokens, above which [`length_penalty`](Self::length_penalty)
    /// starts to apply. Below it a document is untouched.
    ///
    /// Measured in tantivy's *fieldnorm* units, which are a lossy 8-bit
    /// encoding of the token count — roughly logarithmic above 40, so this is
    /// a coarse threshold and precision beyond a couple of significant figures
    /// is imaginary.
    pub length_pivot: u32,

    /// How hard to penalise a document longer than the pivot. `0.0` disables
    /// the penalty entirely.
    ///
    /// The score is divided by `1 + penalty * ln(len / pivot)`, so the cost
    /// grows with the *logarithm* of how far past the pivot a document is: a
    /// page twice the pivot and a page thirty times it are separated, but a
    /// merely long page is not destroyed.
    pub length_penalty: f32,

    /// Which words to drop from a typed query before it is parsed.
    pub stopwords: StopwordPolicy,
}

/// Which words a query gives up before it is run.
///
/// Dropping happens on the **query**, never on the index. That distinction is
/// the whole reason this is tunable: the index still contains every word, so
/// the inverse document frequencies are unchanged and a policy can be changed,
/// or reverted, without rebuilding anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopwordPolicy {
    /// Keep every word.
    None,
    /// Drop interrogatives and the auxiliaries that go with them — `how`,
    /// `what`, `do`, `can`, `should`. Aimed squarely at "how do I …" queries
    /// without touching the vocabulary of a symbol lookup.
    Questions,
    /// [`Questions`](Self::Questions) plus ordinary English function words —
    /// articles, prepositions, pronouns, copulas.
    Function,
}

impl Ranking {
    /// The measured configuration, from `sweep_ranking_parameters` in
    /// `tests/relevance.rs` on 2026-07-29 — coordinate descent on MRR over
    /// 207 queries and 339 documents, constrained to never rank `symbol`
    /// queries worse than the untuned ranker did.
    ///
    /// | | before | tuned |
    /// |---|---|---|
    /// | MRR | 0.7489 | **0.8245** |
    /// | recall@1 | 0.6377 | **0.7585** |
    /// | recall@3 | 0.8357 | **0.8744** |
    /// | `symbol` MRR | 0.8384 | **0.8792** |
    /// | `natural` MRR | 0.2419 | **0.4596** |
    ///
    /// **recall@3 is still short of the Stage 2 exit gate's 0.90**, by roughly
    /// five queries. Ranking alone did not close it and is not going to: the
    /// sweep converged, and a second objective run maximising recall@3
    /// directly reached only 0.8792 while giving up MRR and symbol accuracy to
    /// get there. What remains is S2-5 (`misspelling` is 0.2500 recall@1 by
    /// design until fuzzy matching exists) and better labels, not more tuning.
    ///
    /// Two of these values look wrong and are not:
    ///
    /// - **`title` is below `body`.** A title is a handful of tokens, so BM25's
    ///   own length normalisation already multiplies it heavily; an explicit
    ///   boost on top of that was double-counting, which is why 3.0 lost to
    ///   0.75 in every pass.
    /// - **`code` is below `headers`.** Measured, and consistent with the
    ///   earlier finding that removing the code field entirely is nearly a
    ///   wash — on these platforms method names are *also* headings.
    pub const TUNED: Self = Self {
        title: 0.75,
        headers: 2.0,
        body: 1.0,
        code: 1.0,
        length_pivot: 2_000,
        length_penalty: 0.4,
        stopwords: StopwordPolicy::Function,
    };

    /// Field boosts only, with length penalty and stopwords off.
    ///
    /// This is what ranking looked like before S2-4, and the sweep uses it as
    /// the "change nothing" row so that every reported delta has something to
    /// be a delta from.
    pub const UNTUNED: Self = Self {
        title: 3.0,
        headers: 2.0,
        body: 1.0,
        code: 1.5,
        length_pivot: 0,
        length_penalty: 0.0,
        stopwords: StopwordPolicy::None,
    };
}

impl Default for Ranking {
    fn default() -> Self {
        Self::TUNED
    }
}

/// Divide a score by the length penalty for a document of `fieldnorm` tokens.
///
/// Total on every input the collector can hand it, which matters because it
/// runs per matched document: a `pivot` of 0, a `fieldnorm` of 0, and a
/// negative strength all return the score unchanged rather than producing an
/// infinity or a NaN. A NaN here would not error — it would silently sort
/// wherever `f32::partial_cmp` put it, which is a ranking bug with no symptom
/// beyond "results look wrong sometimes".
pub(super) fn length_scaled(score: Score, fieldnorm: u32, pivot: u32, strength: f32) -> Score {
    if strength <= 0.0 || pivot == 0 || fieldnorm <= pivot {
        return score;
    }
    let ratio = fieldnorm as f32 / pivot as f32;
    score / (1.0 + strength * ratio.ln())
}

/// Interrogatives and their auxiliaries.
///
/// Deliberately short. Every word dropped is a word a user typed, and the
/// justification for each of these is that it appears in a *question frame*
/// rather than in the answer: "how do I add a dependency" is looking for
/// `add dependency`, and `how` matches the FAQ page 400 times.
const QUESTION_WORDS: &[&str] = &[
    "how", "what", "when", "where", "which", "who", "whom", "why", "do", "does", "did", "doing",
    "can", "could", "should", "would", "shall", "will", "may", "might", "must",
];

/// Ordinary function words: articles, prepositions, pronouns, copulas.
///
/// Note what is **not** here. `in`, `is`, `not`, `and`, `or`, `for`, `if`,
/// `return`, `type` and `with` are all keywords or operators in one of the
/// indexed languages, and a documentation search that cannot look up Python's
/// `in` operator has traded a real capability for a marginal one. The eval
/// sweep is the arbiter, but the bias is deliberate.
const FUNCTION_WORDS: &[&str] = &[
    "a", "an", "the", "of", "to", "from", "at", "by", "on", "into", "onto", "about", "i", "me",
    "my", "we", "us", "our", "you", "your", "it", "its", "this", "that", "these", "those", "am",
    "are", "was", "were", "be", "been", "being", "there", "here", "then", "than", "some", "any",
];

impl StopwordPolicy {
    /// Whether `word` — already lowercase — is dropped under this policy.
    fn drops(self, word: &str) -> bool {
        match self {
            Self::None => false,
            Self::Questions => QUESTION_WORDS.contains(&word),
            Self::Function => QUESTION_WORDS.contains(&word) || FUNCTION_WORDS.contains(&word),
        }
    }

    /// Remove stopwords from an already-neutralised query string.
    ///
    /// Two refusals to act, both of which turn a helpful transformation into a
    /// broken search if they are omitted:
    ///
    /// - **A query containing a quote is returned untouched.** The user asked
    ///   for a phrase, and a phrase with a hole in it matches nothing —
    ///   `"how to install"` would become `"to install"` and stop matching the
    ///   text it was quoted from.
    /// - **A query that is *entirely* stopwords is returned untouched.**
    ///   Someone searching `how to` gets whatever that finds; they do not get
    ///   an empty query, which finds nothing and looks like a broken index.
    pub(super) fn apply(self, query: &str) -> String {
        if self == Self::None || query.contains('"') {
            return query.to_owned();
        }

        let mut kept = String::with_capacity(query.len());
        for word in query.split_whitespace() {
            if self.drops(&word.to_lowercase()) {
                continue;
            }
            if !kept.is_empty() {
                kept.push(' ');
            }
            kept.push_str(word);
        }

        if kept.is_empty() {
            return query.to_owned();
        }
        kept
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_document_at_or_below_the_pivot_is_untouched() {
        assert_eq!(length_scaled(1.0, 0, 2_000, 0.6), 1.0);
        assert_eq!(length_scaled(1.0, 1_999, 2_000, 0.6), 1.0);
        assert_eq!(length_scaled(1.0, 2_000, 2_000, 0.6), 1.0);
    }

    #[test]
    fn the_penalty_grows_with_the_logarithm_of_excess_length() {
        let twice = length_scaled(1.0, 4_000, 2_000, 0.6);
        let eight_times = length_scaled(1.0, 16_000, 2_000, 0.6);
        assert!(twice < 1.0);
        assert!(eight_times < twice);
        // Logarithmic, not linear: 8x the length is nowhere near 8x the
        // penalty, which is what keeps a merely long page competitive.
        assert!(eight_times > twice / 4.0);
    }

    #[test]
    fn a_disabled_penalty_is_the_identity() {
        for fieldnorm in [0, 1, 100, 100_000] {
            assert_eq!(length_scaled(2.5, fieldnorm, 2_000, 0.0), 2.5);
            assert_eq!(length_scaled(2.5, fieldnorm, 0, 0.6), 2.5);
        }
    }

    #[test]
    fn no_input_produces_a_nan_or_an_infinity() {
        // The collector runs this per matched document and a NaN would not
        // error -- it would sort arbitrarily. Every degenerate combination has
        // to be finite.
        for fieldnorm in [0, 1, u32::MAX] {
            for pivot in [0, 1, u32::MAX] {
                for strength in [-1.0, 0.0, 0.6, 1_000.0] {
                    let scaled = length_scaled(1.0, fieldnorm, pivot, strength);
                    assert!(scaled.is_finite(), "{fieldnorm} {pivot} {strength}");
                }
            }
        }
    }

    #[test]
    fn question_frames_are_dropped_but_content_words_are_not() {
        assert_eq!(
            StopwordPolicy::Questions.apply("how do I add a dependency"),
            "I add a dependency"
        );
        assert_eq!(
            StopwordPolicy::Function.apply("how do I add a dependency"),
            "add dependency"
        );
    }

    #[test]
    fn language_keywords_survive_every_policy() {
        // A documentation reader that cannot look up Python's `in` operator or
        // Rust's `impl Trait for Type` has traded a real capability for a
        // marginal one.
        for policy in [StopwordPolicy::Questions, StopwordPolicy::Function] {
            for query in ["in operator", "if not None", "for loop", "return type"] {
                assert_eq!(policy.apply(query), query, "{policy:?} changed {query:?}");
            }
        }
    }

    #[test]
    fn a_quoted_query_keeps_every_word() {
        // A phrase with a hole in it matches nothing.
        let query = "\"how do I install\"";
        assert_eq!(StopwordPolicy::Function.apply(query), query);
    }

    #[test]
    fn a_query_of_nothing_but_stopwords_survives_intact() {
        // Better to search for "how do I" and find something than to send an
        // empty query and look like a broken index.
        assert_eq!(StopwordPolicy::Function.apply("how do I"), "how do I");
        assert_eq!(
            StopwordPolicy::Questions.apply("what should"),
            "what should"
        );
    }

    #[test]
    fn dropping_is_case_insensitive_but_does_not_change_case() {
        assert_eq!(StopwordPolicy::Questions.apply("HOW do Foo"), "Foo");
        assert_eq!(StopwordPolicy::None.apply("HOW do Foo"), "HOW do Foo");
    }
}
