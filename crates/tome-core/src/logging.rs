//! Local logging: stderr for the person, a rotated file for the bug report.
//!
//! The PRD's file layout has named `~/Library/Application Support/Tome/logs/`
//! since the beginning and `Paths::ensure_created` has been making the
//! directory since S0-3, but nothing ever wrote to it — so "errors are logged
//! locally for debugging" (P5-004) was true of the directory and false of the
//! logs. This module is what makes it true.
//!
//! Three constraints shape it, and each rules out the obvious implementation:
//!
//! 1. **No telemetry, ever.** Nothing here leaves the machine. The file exists
//!    so that a person can read it, or attach it to an issue *themselves* —
//!    `tome debug report` redacts it for exactly that.
//! 2. **Log lines must carry no reading history.** Page paths, search queries
//!    and note text are what someone reads; a log file is the easiest place
//!    for them to leak, and unlike an error message a log line is written
//!    without anyone reading it back. `crate::error` already forbids user
//!    content in messages; this module refuses to widen that.
//! 3. **Two processes append to one file.** The app and the CLI share a
//!    library (ADR-0002) and can run at once. Each event is therefore
//!    assembled in memory and written with a single `write_all` on an
//!    `O_APPEND` handle, which the kernel does not interleave. A writer that
//!    emitted an event in several small writes would produce a file with
//!    fields from two processes on one line — legible enough to look fine.
//!
//! Rotation is by day, with a retention window, both applied at startup: a
//! long-running app would otherwise hold one day's file open forever, so the
//! date is re-checked on every write.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{Duration, NaiveDate, Utc};

/// How many days of logs to keep. The PRD says 7-day retention.
pub const RETENTION_DAYS: i64 = 7;

const PREFIX: &str = "tome-";
const SUFFIX: &str = ".log";

fn file_for(dir: &Path, date: NaiveDate) -> PathBuf {
    dir.join(format!("{PREFIX}{date}{SUFFIX}"))
}

/// A day-rotated append handle.
///
/// Implements [`tracing_subscriber::fmt::MakeWriter`] via [`LogWriter`], which
/// buffers one event and appends it whole.
pub struct DailyFile {
    dir: PathBuf,
    open: Mutex<Option<(NaiveDate, File)>>,
}

impl DailyFile {
    /// Name a log directory. **Nothing is created until something is logged.**
    ///
    /// That laziness is load-bearing, not an optimisation: `tome search` on a
    /// machine that has pulled nothing must exit 0 and create no library, and
    /// a test asserts it. A logger that made its directory at startup would
    /// turn every read-only command into one that writes.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            open: Mutex::new(None),
        }
    }

    /// Append one already-formatted event.
    fn append(&self, bytes: &[u8]) {
        let today = Utc::now().date_naive();
        let Ok(mut slot) = self.open.lock() else {
            // A poisoned lock means another thread panicked mid-log. Dropping
            // the line is the correct response: logging must not be able to
            // turn one panic into a second one.
            return;
        };
        let needs_open = !matches!(&*slot, Some((date, _)) if *date == today);
        if needs_open {
            // Created here rather than at startup -- see `new`. A failure is
            // silent: losing the log is not a reason to fail the command the
            // log was describing.
            if std::fs::create_dir_all(&self.dir).is_err() {
                return;
            }
            let Ok(file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_for(&self.dir, today))
            else {
                return;
            };
            // Pruning happens on the first write and again whenever the day
            // rolls over, which is the only moment a long-running app would
            // otherwise never reach.
            prune(&self.dir, today);
            *slot = Some((today, file));
        }
        if let Some((_, file)) = slot.as_mut() {
            // One `write_all`, deliberately: see the module docs.
            let _ = file.write_all(bytes);
        }
    }
}

/// Delete log files older than [`RETENTION_DAYS`].
///
/// Parses the date out of the file name rather than trusting mtime, which a
/// backup restore or an `rsync` resets.
fn prune(dir: &Path, today: NaiveDate) {
    let cutoff = today - Duration::days(RETENTION_DAYS);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(date) = name
            .strip_prefix(PREFIX)
            .and_then(|rest| rest.strip_suffix(SUFFIX))
            .and_then(|date| date.parse::<NaiveDate>().ok())
        else {
            // Not one of ours. Never delete a file this module did not write.
            continue;
        };
        if date < cutoff {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// One event's worth of bytes, appended whole when dropped.
pub struct LogWriter<'a> {
    target: &'a DailyFile,
    buffer: Vec<u8>,
}

impl Write for LogWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for LogWriter<'_> {
    fn drop(&mut self) {
        if !self.buffer.is_empty() {
            self.target.append(&self.buffer);
        }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for DailyFile {
    type Writer = LogWriter<'a>;
    fn make_writer(&'a self) -> Self::Writer {
        LogWriter {
            target: self,
            buffer: Vec::new(),
        }
    }
}

/// Both destinations at once: the person's terminal and the log file.
///
/// A layered subscriber could filter the two differently. It deliberately does
/// not: the value of the file is that it says exactly what the user saw, and a
/// file recording more than the terminal is a file recording things nobody
/// decided were safe to record.
pub struct Tee<W> {
    pub stderr: W,
    pub file: DailyFile,
}

/// The stderr half and the file half of one event.
pub struct TeeWriter<'a, W> {
    stderr: W,
    file: LogWriter<'a>,
}

impl<W: Write> Write for TeeWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = self.file.write(buf);
        self.stderr.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stderr.flush()
    }
}

impl<'a, M> tracing_subscriber::fmt::MakeWriter<'a> for Tee<M>
where
    M: tracing_subscriber::fmt::MakeWriter<'a>,
{
    type Writer = TeeWriter<'a, M::Writer>;
    fn make_writer(&'a self) -> Self::Writer {
        TeeWriter {
            stderr: self.stderr.make_writer(),
            file: LogWriter {
                target: &self.file,
                buffer: Vec::new(),
            },
        }
    }
}

/// Log to stderr and to `paths.logs_dir()`.
///
/// Returns the composed writer rather than installing a subscriber: the CLI
/// and the app each build their own (different filters, and the CLI's stdout
/// is reserved for JSON-RPC), and a function that installs a global
/// subscriber can only be called once per process — which makes it untestable
/// and makes a second call a silent no-op.
pub fn to_stderr_and_file(paths: &crate::Paths) -> Tee<fn() -> std::io::Stderr> {
    Tee {
        stderr: std::io::stderr as fn() -> std::io::Stderr,
        file: DailyFile::new(paths.logs_dir()),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use tracing_subscriber::fmt::MakeWriter;

    #[test]
    fn events_are_appended_whole() {
        let dir = tempfile::tempdir().expect("tempdir");
        let daily = DailyFile::new(dir.path());

        // Several `write` calls, one event: this is what the formatter does,
        // and the file must not receive them separately or two processes
        // interleave mid-line.
        {
            let mut w = daily.make_writer();
            w.write_all(b"first ").expect("write");
            w.write_all(b"event\n").expect("write");
        }
        {
            let mut w = daily.make_writer();
            w.write_all(b"second event\n").expect("write");
        }

        let today = Utc::now().date_naive();
        let body = std::fs::read_to_string(file_for(dir.path(), today)).expect("read");
        assert_eq!(body, "first event\nsecond event\n");
    }

    #[test]
    fn pruning_removes_old_logs_and_nothing_else() {
        let dir = tempfile::tempdir().expect("tempdir");
        let today = NaiveDate::from_ymd_opt(2026, 7, 30).expect("date");

        let write = |name: &str| std::fs::write(dir.path().join(name), b"x").expect("write");
        write("tome-2026-07-30.log"); // today
        write("tome-2026-07-24.log"); // 6 days old — inside the window
        write("tome-2026-07-22.log"); // 8 days old — outside it
        write("tome-not-a-date.log"); // not ours
        write("notes.txt"); // definitely not ours

        prune(dir.path(), today);

        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .expect("read_dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec![
                "notes.txt".to_owned(),
                "tome-2026-07-24.log".to_owned(),
                "tome-2026-07-30.log".to_owned(),
                "tome-not-a-date.log".to_owned(),
            ],
            "only dated files past the window may be deleted"
        );
    }

    #[test]
    fn the_tee_reaches_both_halves() {
        let dir = tempfile::tempdir().expect("tempdir");
        // A writer that records rather than printing, so the test can assert
        // the stderr half arrived without polluting the test run's output.
        #[derive(Clone, Default)]
        struct Captured(std::sync::Arc<Mutex<Vec<u8>>>);
        impl Write for Captured {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                if let Ok(mut inner) = self.0.lock() {
                    inner.extend_from_slice(buf);
                }
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl<'a> MakeWriter<'a> for Captured {
            type Writer = Captured;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let captured = Captured::default();
        let tee = Tee {
            stderr: captured.clone(),
            file: DailyFile::new(dir.path()),
        };
        {
            let mut w = tee.make_writer();
            w.write_all(b"both halves\n").expect("write");
        }

        let seen = captured.0.lock().expect("lock").clone();
        assert_eq!(String::from_utf8_lossy(&seen), "both halves\n");
        let today = Utc::now().date_naive();
        assert_eq!(
            std::fs::read_to_string(file_for(dir.path(), today)).expect("read"),
            "both halves\n"
        );
    }
}
