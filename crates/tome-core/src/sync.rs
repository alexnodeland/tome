//! When a source is due to be re-fetched (P4-018).
//!
//! **This module decides; it never fetches.** That split is the whole design.
//! P4-018's own technical note records why: the original sketch polled every
//! watched package's registry on a 60-second interval, which for 30 watched
//! crates is ~43 000 requests a day to crates.io for information that changes
//! weekly — abusive, and a direct contradiction of the NFR "no background
//! network activity without user action". A pure `due` function is also
//! testable without a clock or a socket, which is why every case below is a
//! unit test rather than a hope.
//!
//! **Missed schedules coalesce; they are not replayed.** A laptop closed for
//! six weeks owes one weekly sync on waking, not six. That falls out of
//! comparing `last_synced` to a threshold rather than counting elapsed
//! periods, and a test pins it.
//!
//! **`Watch` is deliberately not implemented.** DEC-006 — fetch eagerly, or
//! notify? — is open, and it is the owner's call. [`due`] returns
//! [`Due::WatchUndecided`] for it rather than guessing: a caller that treats
//! that as "not due" is behaving correctly and conservatively, and one that
//! wants to act on it has to come here and read this.

use chrono::{DateTime, Duration, Utc};

use crate::model::{Schedule, SyncConfig, SyncStrategy};

/// Why a source is, or is not, due.
///
/// An enum rather than a `bool` because the *reasons* differ in what a caller
/// should do about them: "never pulled" wants a first pull, "pinned" wants
/// nothing ever, and `WatchUndecided` wants a decision from a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Due {
    /// Fetch it.
    Now,
    /// A scheduled source whose interval has not elapsed.
    NotYet,
    /// Only a user-initiated `tome pull` fetches this source.
    OnlyManually,
    /// `pin_version` is set: never auto-update, whatever the strategy says.
    Pinned,
    /// A `watch` source. **DEC-006 is unresolved** — this is not a "no", it
    /// is "nobody has decided yet". See the module docs.
    WatchUndecided,
}

impl Due {
    /// Whether an automatic sync pass should fetch this source now.
    ///
    /// `WatchUndecided` is **false** here, deliberately: acting on an
    /// undecided policy would resolve DEC-006 by accident, and the
    /// conservative direction is the one that makes no network request.
    pub fn should_fetch(self) -> bool {
        matches!(self, Self::Now)
    }
}

/// What triggered this evaluation. The same source answers differently at app
/// launch than on a background tick, and conflating the two is how
/// `on_launch` sources end up syncing every 15 minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The app or CLI just started.
    Launch,
    /// A periodic scheduler tick.
    Tick,
}

/// Whether `config` is due to be fetched, given when it was last synced.
///
/// `now` is a parameter rather than a call to `Utc::now()` so the tests state
/// their own clock — a scheduling function that reads the wall clock can only
/// be tested by waiting.
pub fn due(
    config: &SyncConfig,
    last_synced: Option<DateTime<Utc>>,
    trigger: Trigger,
    now: DateTime<Utc>,
) -> Due {
    // Pinning wins over everything, including a schedule. "Even a scheduled
    // sync only revalidates the pinned version" (`SyncConfig::pin_version`),
    // and revalidation is not this function's business.
    if config.pin_version {
        return Due::Pinned;
    }

    match &config.strategy {
        SyncStrategy::Manual => Due::OnlyManually,

        // Launch means launch. On a tick this is not due — otherwise every
        // background tick would re-fetch every on-launch source, which is the
        // opposite of what the strategy asks for.
        SyncStrategy::OnLaunch => match trigger {
            Trigger::Launch => Due::Now,
            Trigger::Tick => Due::NotYet,
        },

        SyncStrategy::Scheduled { schedule } => {
            let threshold = interval(*schedule);
            match last_synced {
                // Never synced is due under either trigger: a configured
                // source with no content is the state a user most wants
                // fixed, and it costs one crawl, not one per missed period.
                None => Due::Now,
                Some(last) => {
                    // Coalescing lives in this comparison. Six weeks late is
                    // one sync, because the question is "has the interval
                    // elapsed", not "how many intervals elapsed".
                    //
                    // A `last_synced` in the future — clock skew, or a file
                    // synced from a machine ahead of this one — yields a
                    // negative duration and therefore "not yet", which is
                    // the safe direction: it delays a fetch rather than
                    // looping on one.
                    if now - last >= threshold {
                        Due::Now
                    } else {
                        Due::NotYet
                    }
                }
            }
        }

        SyncStrategy::Watch { .. } => Due::WatchUndecided,
    }
}

/// How long a schedule waits between syncs.
///
/// Monthly is 30 days rather than a calendar month: documentation does not
/// care which month it is, and "the 31st" has no meaning in February.
fn interval(schedule: Schedule) -> Duration {
    match schedule {
        Schedule::Daily => Duration::days(1),
        Schedule::Weekly => Duration::weeks(1),
        Schedule::Monthly => Duration::days(30),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn at(iso: &str) -> DateTime<Utc> {
        iso.parse().expect("valid RFC 3339 timestamp")
    }

    fn config(strategy: SyncStrategy) -> SyncConfig {
        SyncConfig {
            strategy,
            pin_version: false,
        }
    }

    fn scheduled(schedule: Schedule) -> SyncConfig {
        config(SyncStrategy::Scheduled { schedule })
    }

    #[test]
    fn manual_is_never_automatically_due() {
        for trigger in [Trigger::Launch, Trigger::Tick] {
            let verdict = due(
                &config(SyncStrategy::Manual),
                None,
                trigger,
                at("2026-07-30T00:00:00Z"),
            );
            assert_eq!(verdict, Due::OnlyManually);
            assert!(!verdict.should_fetch());
        }
    }

    #[test]
    fn on_launch_fires_at_launch_and_not_on_every_tick() {
        let config = config(SyncStrategy::OnLaunch);
        let now = at("2026-07-30T00:00:00Z");
        assert_eq!(due(&config, None, Trigger::Launch, now), Due::Now);
        // The bug this prevents: an on-launch source re-fetched every 15
        // minutes for as long as the app stays open.
        assert_eq!(due(&config, None, Trigger::Tick, now), Due::NotYet);
    }

    #[test]
    fn a_schedule_that_has_not_elapsed_is_not_due() {
        let config = scheduled(Schedule::Weekly);
        let last = at("2026-07-29T00:00:00Z");
        let verdict = due(
            &config,
            Some(last),
            Trigger::Tick,
            at("2026-07-30T00:00:00Z"),
        );
        assert_eq!(verdict, Due::NotYet);
    }

    #[test]
    fn a_schedule_is_due_the_moment_the_interval_elapses() {
        let last = at("2026-07-23T00:00:00Z");
        // Exactly one week later: due. `>=`, not `>`, so a daily source at
        // the same time each day does not drift a day later every day.
        assert_eq!(
            due(
                &scheduled(Schedule::Weekly),
                Some(last),
                Trigger::Tick,
                at("2026-07-30T00:00:00Z")
            ),
            Due::Now
        );
        assert_eq!(
            due(
                &scheduled(Schedule::Weekly),
                Some(last),
                Trigger::Tick,
                at("2026-07-29T23:59:59Z")
            ),
            Due::NotYet
        );
    }

    #[test]
    fn missed_schedules_coalesce_into_one() {
        // Six weeks closed. This must be ONE sync, not six — the property
        // P4-018 names, and the reason `due` answers a yes/no rather than
        // handing back a count of missed periods.
        let last = at("2026-06-18T00:00:00Z");
        let now = at("2026-07-30T00:00:00Z");
        assert_eq!(
            due(&scheduled(Schedule::Weekly), Some(last), Trigger::Tick, now),
            Due::Now
        );
        // And once it has synced, it is not due again until next week — the
        // observable meaning of "one, not six".
        assert_eq!(
            due(&scheduled(Schedule::Weekly), Some(now), Trigger::Tick, now),
            Due::NotYet
        );
    }

    #[test]
    fn a_never_synced_scheduled_source_is_due() {
        assert_eq!(
            due(
                &scheduled(Schedule::Monthly),
                None,
                Trigger::Tick,
                at("2026-07-30T00:00:00Z")
            ),
            Due::Now
        );
    }

    #[test]
    fn a_last_synced_in_the_future_delays_rather_than_loops() {
        // Clock skew, or a config synced from a machine ahead of this one.
        // The wrong behaviour here is a fetch loop; the right one is to wait.
        let verdict = due(
            &scheduled(Schedule::Daily),
            Some(at("2026-08-30T00:00:00Z")),
            Trigger::Tick,
            at("2026-07-30T00:00:00Z"),
        );
        assert_eq!(verdict, Due::NotYet);
    }

    #[test]
    fn pinning_beats_every_strategy_and_every_trigger() {
        for strategy in [
            SyncStrategy::Manual,
            SyncStrategy::OnLaunch,
            SyncStrategy::Scheduled {
                schedule: Schedule::Daily,
            },
            SyncStrategy::Watch {
                source: "crates:serde".into(),
            },
        ] {
            let config = SyncConfig {
                strategy,
                pin_version: true,
            };
            for trigger in [Trigger::Launch, Trigger::Tick] {
                let verdict = due(&config, None, trigger, at("2026-07-30T00:00:00Z"));
                assert_eq!(verdict, Due::Pinned, "pinned sources never auto-update");
                assert!(!verdict.should_fetch());
            }
        }
    }

    #[test]
    fn watch_is_undecided_and_does_not_fetch() {
        // DEC-006 is open. This test exists to fail loudly if someone
        // implements `watch` here instead of deciding it first.
        let config = config(SyncStrategy::Watch {
            source: "crates:serde".into(),
        });
        let verdict = due(&config, None, Trigger::Launch, at("2026-07-30T00:00:00Z"));
        assert_eq!(verdict, Due::WatchUndecided);
        assert!(
            !verdict.should_fetch(),
            "an undecided policy must not make network requests"
        );
    }
}
