//! The API bearer token (P4-012): generation, storage, rotation, validation.
//!
//! **Loopback is not a trust boundary.** Every process on the machine and
//! every web page in the user's browser can originate a loopback request, so
//! the token is required on every API request with no bypass and no opt-out —
//! that decision lives in the serve middleware; this module just makes the
//! token exist, persist, and compare safely.
//!
//! Storage: the macOS Keychain (via `/usr/bin/security`, invoked by absolute
//! path with an argument vector — the same rule as `mandoc` in man ingest:
//! resolving an executable through `PATH` is running arbitrary code). When
//! `TOME_HOME` is set — tests, throwaway libraries — the token lives in a
//! `0600` file under the state root instead, because a test run must not
//! write to the user's real Keychain.
//!
//! `TOME_API_TOKEN`, when set, overrides both stores for the *server's*
//! expected token — the hermetic-test hook, and useful for scripts. It is an
//! override of where the secret comes from, not a second accepted token.

use anyhow::{Context, Result};
use tome_core::Paths;

const KEYCHAIN_SERVICE: &str = "com.alexnodeland.tome.api-token";
const KEYCHAIN_ACCOUNT: &str = "tome";
const SECURITY: &str = "/usr/bin/security";

/// Holds only the SHA-256 of the expected token. The server process never
/// needs the token itself after startup, so it does not keep it — a memory
/// disclosure then leaks a hash, not a credential.
pub(crate) struct TokenValidator {
    expected_digest: [u8; 32],
}

impl TokenValidator {
    pub fn new(token: &str) -> Self {
        Self {
            expected_digest: tome_core::hash::sha256(token.as_bytes()),
        }
    }

    /// Compare a presented token in constant time.
    ///
    /// Both sides are hashed first, so the comparison operates on
    /// fixed-length digests of attacker-*uncontrollable* relationship — the
    /// standard way to make `==` on secrets safe: a timing signal on the
    /// digest bytes tells the attacker nothing about the token prefix.
    /// The fold is still branch-free out of caution.
    pub fn validate(&self, presented: &str) -> bool {
        let presented_digest = tome_core::hash::sha256(presented.as_bytes());
        self.expected_digest
            .iter()
            .zip(presented_digest.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }
}

/// The token, from wherever it lives — generating and storing one on first
/// use.
pub(crate) fn load_or_create(paths: &Paths) -> Result<String> {
    if let Ok(token) = std::env::var("TOME_API_TOKEN") {
        if !token.trim().is_empty() {
            return Ok(token.trim().to_owned());
        }
    }
    if let Some(existing) = read(paths)? {
        return Ok(existing);
    }
    let token = generate()?;
    write(paths, &token)?;
    Ok(token)
}

/// Replace the token (`tome config rotate-token`). The old token stops
/// working the next time the server reads the store — a running server keeps
/// its startup token until restarted, and `rotate-token` says so.
pub(crate) fn rotate(paths: &Paths) -> Result<String> {
    let token = generate()?;
    write(paths, &token)?;
    Ok(token)
}

/// 256 bits from the system CSPRNG, hex-encoded.
///
/// Read straight from `/dev/urandom`: no `rand` dependency for one read, and
/// on macOS urandom is the CSPRNG.
fn generate() -> Result<String> {
    use std::io::Read;
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .context("reading /dev/urandom")?;
    Ok(tome_core::hash::hex(&bytes))
}

fn use_file_store() -> bool {
    // TOME_HOME set means "not the user's real library" — tests and
    // throwaway environments — and those must not touch the real Keychain.
    std::env::var_os(tome_core::paths::TOME_HOME_ENV).is_some()
}

fn token_file(paths: &Paths) -> std::path::PathBuf {
    paths.state_root().join("api-token")
}

fn read(paths: &Paths) -> Result<Option<String>> {
    if use_file_store() {
        return match std::fs::read_to_string(token_file(paths)) {
            Ok(token) => Ok(Some(token.trim().to_owned()).filter(|t| !t.is_empty())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("reading the API token file"),
        };
    }
    let out = std::process::Command::new(SECURITY)
        .args([
            "find-generic-password",
            "-a",
            KEYCHAIN_ACCOUNT,
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
        ])
        .output()
        .context("running /usr/bin/security")?;
    if !out.status.success() {
        // Not found is the common case on first run; `security` exits 44.
        return Ok(None);
    }
    let token = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    Ok(Some(token).filter(|t| !t.is_empty()))
}

fn write(paths: &Paths, token: &str) -> Result<()> {
    if use_file_store() {
        paths.ensure_created()?;
        let file = token_file(paths);
        std::fs::write(&file, token).context("writing the API token file")?;
        tome_core::paths::restrict_file(&file)?;
        return Ok(());
    }
    // `-U` updates in place on rotation. The token goes through an argument
    // vector, never a shell.
    let status = std::process::Command::new(SECURITY)
        .args([
            "add-generic-password",
            "-U",
            "-a",
            KEYCHAIN_ACCOUNT,
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
            token,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("running /usr/bin/security")?;
    anyhow::ensure!(
        status.success(),
        "could not store the token in the Keychain"
    );
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn validation_accepts_the_token_and_only_the_token() {
        let validator = TokenValidator::new("correct-horse");
        assert!(validator.validate("correct-horse"));
        assert!(!validator.validate("correct-horsf"));
        assert!(!validator.validate("correct-hors"));
        assert!(!validator.validate(""));
    }

    #[test]
    fn generated_tokens_are_long_and_distinct() {
        let a = generate().expect("generate");
        let b = generate().expect("generate");
        assert_eq!(a.len(), 64, "256 bits hex-encoded");
        assert_ne!(a, b);
    }
}
