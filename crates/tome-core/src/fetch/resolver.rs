//! The SSRF check, installed as ureq's name resolver.
//!
//! Delegates the actual DNS lookup to ureq's [`DefaultResolver`], then drops
//! every address that [`ssrf::classify`] does not permit under the policy.
//! Because the addresses this returns are the exact addresses ureq's
//! connector dials, there is no second resolution and therefore no
//! DNS-rebinding window — the check and the connection see the same answer.
//!
//! This uses ureq's `unversioned` resolver API, which the crate documents as
//! not following semver. That is an accepted coupling: pinning `ureq = "3"`
//! and having one integration test that actually connects is the tripwire if
//! a future 3.x changes it. The alternative — resolving ourselves and
//! reconnecting by IP — reintroduces exactly the TOCTOU gap this exists to
//! close.

use std::fmt;

use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

use super::ssrf::AddressPolicy;

pub struct GuardResolver {
    inner: DefaultResolver,
    policy: AddressPolicy,
}

impl GuardResolver {
    pub fn new(policy: AddressPolicy) -> Self {
        Self {
            inner: DefaultResolver::default(),
            policy,
        }
    }
}

impl fmt::Debug for GuardResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuardResolver")
            .field("policy", &self.policy)
            .finish()
    }
}

impl Resolver for GuardResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let resolved = self.inner.resolve(uri, config, timeout)?;

        let mut permitted = self.empty();
        let mut blocked = 0usize;
        for addr in resolved.iter() {
            if self.policy.permits(addr.ip()) {
                permitted.push(*addr);
            } else {
                blocked += 1;
                tracing::warn!(
                    ip = %addr.ip(),
                    "SSRF filter blocked a resolved address"
                );
            }
        }

        if permitted.is_empty() {
            // Every address was blocked (or DNS returned none). Refuse the
            // connection. ureq surfaces this as HostNotFound; the fetcher
            // maps that to Error::BlockedByFilter so the caller sees the
            // real reason. A partially-blocked set is NOT an error: if one
            // A record is public we may reach it, and the blocked ones are
            // simply not dialed — a rebinding attacker cannot force a bad
            // address to be chosen because none of them remain in the list.
            if blocked > 0 {
                tracing::warn!("SSRF filter blocked every resolved address for the host");
            }
            return Err(ureq::Error::HostNotFound);
        }

        Ok(permitted)
    }
}
