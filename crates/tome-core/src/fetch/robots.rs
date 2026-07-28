//! `robots.txt` parsing and matching — the subset of RFC 9309 Tome needs,
//! plus the non-standard `Crawl-delay`, which the PRD commits to obeying.
//!
//! Hand-rolled rather than a crate because the behaviour is a product
//! commitment ("obeyed by default, non-overridable for registry configs")
//! and must therefore be *testable in this repo* against the fixture
//! server, fuzzable, and readable in one sitting. The rules implemented:
//!
//! - Groups of `User-agent:` lines followed by `Allow:`/`Disallow:` rules;
//!   the group chosen is the one whose user-agent token is the longest
//!   match for ours (`tome`), falling back to `*`.
//! - Longest-path-match wins; on equal length, `Allow` wins (RFC 9309 §2.2.2).
//! - `*` matches any character sequence; `$` anchors the end of the path.
//! - An empty `Disallow:` value allows everything (the classic idiom).
//! - Matching is case-sensitive on paths, case-insensitive on field names
//!   and user-agent tokens, per the RFC.
//! - Input beyond 500 KiB is ignored (the RFC's minimum processing bound) —
//!   a hostile robots.txt must not become a memory or CPU sink.
//!
//! Deliberate simplification: rule paths and request paths are compared as
//! written (both already percent-encoded in practice); the RFC's
//! decode-except-reserved dance is skipped. If a real site's rules misfire
//! over this, the golden corpus is where it will show up.

use std::time::Duration;

/// The policy one `robots.txt` expresses for our user agent.
#[derive(Debug, Clone, PartialEq)]
pub struct RobotsPolicy {
    rules: Vec<Rule>,
    crawl_delay: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq)]
struct Rule {
    allow: bool,
    path: String,
}

/// The product token robots.txt groups are matched against.
pub const USER_AGENT_TOKEN: &str = "tome";

/// RFC 9309 requires processing at least 500 KiB; everything past that is
/// ignored rather than parsed.
const MAX_INPUT: usize = 500 * 1024;

impl RobotsPolicy {
    /// The policy when no robots.txt exists (or it returned 4xx): everything
    /// is allowed.
    pub fn allow_all() -> Self {
        Self {
            rules: Vec::new(),
            crawl_delay: None,
        }
    }

    /// The policy when robots.txt could not be retrieved (5xx, network
    /// failure): everything is disallowed. Conservative by design — a host
    /// that is failing is not a host to crawl harder.
    pub fn disallow_all() -> Self {
        Self {
            rules: vec![Rule {
                allow: false,
                path: "/".into(),
            }],
            crawl_delay: None,
        }
    }

    /// Parse a robots.txt body, selecting the group that applies to Tome.
    pub fn parse(body: &str) -> Self {
        let body = truncate_at_char_boundary(body, MAX_INPUT);

        // First pass: collect groups as (user-agent tokens, rules, delay).
        struct Group {
            agents: Vec<String>,
            rules: Vec<Rule>,
            crawl_delay: Option<Duration>,
        }
        let mut groups: Vec<Group> = Vec::new();
        // Whether the last meaningful line was a `User-agent`: consecutive
        // user-agent lines share one group (RFC 9309 §2.1).
        let mut in_agent_run = false;

        for line in body.lines() {
            let line = line.split('#').next().unwrap_or("");
            let Some((field, value)) = line.split_once(':') else {
                continue;
            };
            let field = field.trim().to_ascii_lowercase();
            let value = value.trim();

            match field.as_str() {
                "user-agent" => {
                    if !in_agent_run {
                        groups.push(Group {
                            agents: Vec::new(),
                            rules: Vec::new(),
                            crawl_delay: None,
                        });
                        in_agent_run = true;
                    }
                    if let Some(group) = groups.last_mut() {
                        group.agents.push(value.to_ascii_lowercase());
                    }
                }
                "allow" | "disallow" => {
                    in_agent_run = false;
                    if let Some(group) = groups.last_mut() {
                        // An empty Disallow allows everything — represented
                        // by simply not adding a rule. An empty Allow is
                        // equally meaningless.
                        if !value.is_empty() {
                            group.rules.push(Rule {
                                allow: field == "allow",
                                path: value.to_string(),
                            });
                        }
                    }
                }
                "crawl-delay" => {
                    in_agent_run = false;
                    if let Some(group) = groups.last_mut() {
                        if let Ok(secs) = value.parse::<f64>() {
                            if secs.is_finite() && secs >= 0.0 {
                                // Cap at 60s: a hostile or typoed delay must
                                // not stall a sync for hours per page.
                                group.crawl_delay = Some(Duration::from_secs_f64(secs.min(60.0)));
                            }
                        }
                    }
                }
                // sitemap, host, and anything else: not ours to interpret.
                _ => {
                    in_agent_run = false;
                }
            }
        }

        // Group selection: longest user-agent token that is a prefix of (or
        // equal to) ours wins; `*` is the fallback with length zero.
        let mut best: Option<(usize, &Group)> = None;
        for group in &groups {
            for agent in &group.agents {
                let specificity = if agent == "*" {
                    Some(0)
                } else if USER_AGENT_TOKEN.contains(agent.as_str())
                    || agent.contains(USER_AGENT_TOKEN)
                {
                    Some(agent.len())
                } else {
                    None
                };
                if let Some(s) = specificity {
                    if best.is_none_or(|(b, _)| s > b) {
                        best = Some((s, group));
                    }
                }
            }
        }

        match best {
            Some((_, group)) => Self {
                rules: group.rules.clone(),
                crawl_delay: group.crawl_delay,
            },
            None => Self::allow_all(),
        }
    }

    /// May this path be fetched?
    pub fn allows(&self, path: &str) -> bool {
        // A fetch of "/robots.txt" itself is always fine, per the RFC.
        if path == "/robots.txt" {
            return true;
        }
        let mut verdict = true;
        let mut best_len = 0usize;
        for rule in &self.rules {
            if rule_matches(&rule.path, path) {
                // Longest match wins; on a tie, Allow wins, which the `>=`
                // on the allow arm implements.
                let len = rule.path.len();
                if (rule.allow && len >= best_len) || (!rule.allow && len > best_len) {
                    best_len = len;
                    verdict = rule.allow;
                }
            }
        }
        verdict
    }

    /// The group's `Crawl-delay`, if it declared one.
    pub fn crawl_delay(&self) -> Option<Duration> {
        self.crawl_delay
    }
}

/// Match one rule path against a request path. `*` is a wildcard, a trailing
/// `$` anchors the end; everything else is literal, case-sensitive.
///
/// Iterative with explicit backtracking over `*` — the classic recursive
/// formulation is exponential on inputs like `"/a*a*a*a*a*b"` vs `"/aaaaaaa"`,
/// and robots.txt is attacker-supplied.
fn rule_matches(rule: &str, path: &str) -> bool {
    let (rule, anchored) = match rule.strip_suffix('$') {
        Some(stripped) => (stripped, true),
        None => (rule, false),
    };
    let r: Vec<char> = rule.chars().collect();
    let p: Vec<char> = path.chars().collect();

    let (mut ri, mut pi) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None; // (rule idx after *, path idx at *)

    while pi < p.len() {
        if ri < r.len() && (r[ri] == p[pi]) {
            ri += 1;
            pi += 1;
        } else if ri < r.len() && r[ri] == '*' {
            star = Some((ri + 1, pi));
            ri += 1;
        } else if let Some((sri, spi)) = star {
            // Backtrack: let the last * absorb one more character.
            ri = sri;
            pi = spi + 1;
            star = Some((sri, spi + 1));
        } else {
            // Prefix matched entirely and rule is exhausted: match unless
            // the rule was end-anchored.
            return ri == r.len() && !anchored;
        }
    }
    // Path exhausted: remaining rule chars must all be `*`.
    while ri < r.len() && r[ri] == '*' {
        ri += 1;
    }
    ri == r.len()
}

/// Truncate to at most `max` bytes without splitting a UTF-8 character —
/// `body[..max]` panics on multi-byte input, which is exactly the kind of
/// crash a hostile robots.txt would love.
fn truncate_at_char_boundary(body: &str, max: usize) -> &str {
    if body.len() <= max {
        return body;
    }
    let mut end = max;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    &body[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_missing_rules_allow_everything() {
        assert!(RobotsPolicy::parse("").allows("/anything"));
        assert!(RobotsPolicy::allow_all().allows("/anything"));
        assert!(!RobotsPolicy::disallow_all().allows("/anything"));
    }

    #[test]
    fn the_fixture_sites_shape() {
        let policy = RobotsPolicy::parse("User-agent: *\nDisallow: /private/\n");
        assert!(policy.allows("/index.html"));
        assert!(policy.allows("/api/reference.html"));
        assert!(!policy.allows("/private/internal.html"));
    }

    #[test]
    fn longest_match_wins_and_allow_wins_ties() {
        let policy = RobotsPolicy::parse("User-agent: *\nDisallow: /docs/\nAllow: /docs/public/\n");
        assert!(!policy.allows("/docs/secret.html"));
        assert!(policy.allows("/docs/public/index.html"));

        // Equal length: Allow wins.
        let tie = RobotsPolicy::parse("User-agent: *\nDisallow: /a/\nAllow: /a/\n");
        assert!(tie.allows("/a/x"));
    }

    #[test]
    fn empty_disallow_is_allow_all() {
        let policy = RobotsPolicy::parse("User-agent: *\nDisallow:\n");
        assert!(policy.allows("/anything"));
    }

    #[test]
    fn specific_group_beats_star_group() {
        let policy = RobotsPolicy::parse(
            "User-agent: *\nDisallow: /\n\nUser-agent: tome\nDisallow: /private/\n",
        );
        assert!(policy.allows("/docs/index.html"), "the tome group applies");
        assert!(!policy.allows("/private/x"));
    }

    #[test]
    fn consecutive_user_agents_share_a_group() {
        let policy =
            RobotsPolicy::parse("User-agent: googlebot\nUser-agent: tome\nDisallow: /shared/\n");
        assert!(!policy.allows("/shared/x"));
    }

    #[test]
    fn wildcards_and_anchors() {
        let policy = RobotsPolicy::parse("User-agent: *\nDisallow: /*.json$\nDisallow: /tmp*\n");
        assert!(!policy.allows("/api/data.json"));
        assert!(policy.allows("/api/data.json.html"), "$ anchors the end");
        assert!(!policy.allows("/tmp/x"));
        assert!(!policy.allows("/tmpfile"));
        assert!(policy.allows("/team"));
    }

    #[test]
    fn pathological_wildcards_terminate() {
        // The exponential-backtracking shape; must return promptly.
        let rule = "/a*a*a*a*a*a*a*a*a*b";
        let path = format!("/{}", "a".repeat(60));
        assert!(!rule_matches(rule, &path));
    }

    #[test]
    fn robots_txt_itself_is_always_fetchable() {
        let policy = RobotsPolicy::parse("User-agent: *\nDisallow: /\n");
        assert!(policy.allows("/robots.txt"));
        assert!(!policy.allows("/index.html"));
    }

    #[test]
    fn crawl_delay_is_parsed_and_capped() {
        let policy = RobotsPolicy::parse("User-agent: *\nCrawl-delay: 2.5\nDisallow: /x\n");
        assert_eq!(policy.crawl_delay(), Some(Duration::from_secs_f64(2.5)));

        let hostile = RobotsPolicy::parse("User-agent: *\nCrawl-delay: 86400\n");
        assert_eq!(hostile.crawl_delay(), Some(Duration::from_secs(60)));

        let nonsense = RobotsPolicy::parse("User-agent: *\nCrawl-delay: NaN\n");
        assert_eq!(nonsense.crawl_delay(), None);
    }

    #[test]
    fn comments_and_junk_are_ignored() {
        let policy = RobotsPolicy::parse(
            "# a comment\nUser-agent: * # trailing\nDisallow: /x # here too\nnonsense line\nSitemap: https://x/sitemap.xml\n",
        );
        assert!(!policy.allows("/x/y"));
        assert!(policy.allows("/y"));
    }

    #[test]
    fn oversized_input_is_truncated_on_a_char_boundary() {
        // 500 KiB of multi-byte characters right across the boundary; the
        // naive `&body[..MAX]` would panic here.
        let body = "é".repeat(MAX_INPUT); // 2 bytes each
        let policy = RobotsPolicy::parse(&body);
        assert!(policy.allows("/anything"));
    }
}
