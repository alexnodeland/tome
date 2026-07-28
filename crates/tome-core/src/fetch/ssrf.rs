//! SSRF defence (implementation plan S1-5, from P1-008 / security).
//!
//! Tome fetches URLs that ultimately come from users and from the registry.
//! Without a filter, "add this documentation source" is a request to make
//! Tome's process connect anywhere the *process* can reach — cloud metadata
//! endpoints (`169.254.169.254`), localhost admin panels, RFC 1918 intranet
//! hosts. That is a server-side request forgery primitive, and a docs reader
//! has no business connecting to any of them.
//!
//! # Two halves, and why
//!
//! 1. [`classify`] decides whether one resolved [`IpAddr`] is a public
//!    destination. Pure, exhaustive, and where every test and the fuzz
//!    target aim — no network, no timing, just address arithmetic.
//! 2. [`GuardResolver`] installs that check *inside ureq's name
//!    resolution*, so the addresses that pass the check are the exact
//!    addresses the connection uses. This is what closes the DNS-rebinding
//!    window: a naive "resolve, check, then fetch" re-resolves at connect
//!    time and a hostile DNS server can return a public address the first
//!    time and `127.0.0.1` the second. Here there is only one resolution.
//!
//! # What is blocked
//!
//! Everything that is not a *global* unicast address:
//!
//! - loopback (`127.0.0.0/8`, `::1`)
//! - private / RFC 1918 (`10/8`, `172.16/12`, `192.168/16`) and IPv6 ULA
//!   (`fc00::/7`)
//! - link-local (`169.254/16` — which includes the cloud metadata address —
//!   and `fe80::/10`)
//! - CGNAT (`100.64/10`)
//! - unspecified (`0.0.0.0`, `::`), broadcast, documentation and benchmark
//!   ranges, and the rest of the IANA special-purpose registry that is not
//!   globally routable
//! - **every v4-in-v6 embedding** — mapped (`::ffff:127.0.0.1`), compatible
//!   (`::127.0.0.1`), IPv4-translated (`::ffff:0:127.0.0.1`), NAT64
//!   (`64:ff9b::127.0.0.1`), and 6to4 (`2002:7f00:1::`) — unwrapped to its v4
//!   address and classified as v4, so no v6 spelling reaches an internal host
//!   a translator or relay would forward to
//! - **any v6 address outside global unicast `2000::/3`** — the classifier
//!   fails *closed* here rather than enumerating every non-global range, so a
//!   range nobody named (site-local `fec0::/10`, Teredo, unallocated space)
//!   cannot leak through as "Public"
//!
//! Those last two rules exist because an adversarial refute-panel (S1-5's
//! verification gate) defeated the first draft: it reached `169.254.169.254`
//! through the NAT64 prefix and found `fec0::/10` classified Public. The
//! draft named the bad ranges and defaulted to Public; this one unwraps the
//! embeddings and defaults to Forbidden.
//!
//! The default posture blocks all of these. A source config with
//! `allow_insecure: true` — already declared to mean "a host you own", and
//! already the only way to fetch over plain http — additionally permits
//! loopback and private ranges, for an intranet documentation mirror. It
//! never permits link-local: the metadata endpoint is not a documentation
//! host under any configuration.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// The verdict for one address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressClass {
    /// A globally-routable public unicast address — fetch permitted.
    Public,
    /// Loopback or a private/RFC1918/ULA/CGNAT range — permitted only when
    /// the source config set `allow_insecure` (an owned intranet host).
    PrivateOrLoopback,
    /// Link-local (incl. cloud metadata), multicast, unspecified, and every
    /// other non-global special-purpose range — never permitted, under any
    /// configuration.
    Forbidden,
}

/// Classify a resolved address.
///
/// Every v6 spelling that embeds a v4 address is unwrapped to that v4 and
/// classified as v4 first — mapped, compatible, IPv4-translated (SIIT),
/// NAT64 well-known prefix, and 6to4 — so no v6 costume can classify
/// differently from the v4 it will actually reach through a translator or
/// relay. What remains is a real v6 address, and there the rule is
/// **default-deny outside global unicast `2000::/3`**: the SSRF refute-panel
/// showed that naming the bad ranges and defaulting to Public leaks every
/// range nobody remembered to name (site-local `fec0::/10` was the example).
/// Public v6 is entirely within `2000::/3` today; anything outside it is
/// special-purpose, reserved, or unallocated, and a docs host has no reason
/// to live there.
pub fn classify(ip: IpAddr) -> AddressClass {
    match ip {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = embedded_v4(v6) {
                return classify_v4(v4);
            }
            classify_v6(v6)
        }
    }
}

/// The v4 address a v6 address embeds, for every translation/relay form a
/// real network would forward. `None` means "a genuine v6 address".
fn embedded_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    // ::ffff:a.b.c.d — IPv4-mapped.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return Some(v4);
    }
    let s = ip.segments();
    // ::a.b.c.d — deprecated IPv4-compatible. to_ipv4() also matches the
    // mapped form (handled) and :: / ::1, which are v6 specials, not v4.
    if let Some(v4) = ip.to_ipv4() {
        if !ip.is_unspecified() && !ip.is_loopback() {
            return Some(v4);
        }
    }
    // ::ffff:0:a.b.c.d — IPv4-translated (SIIT, RFC 2765): segs 0..4 zero,
    // seg 4 = 0xffff, seg 5 = 0, v4 in the low 32 bits.
    if s[0] == 0 && s[1] == 0 && s[2] == 0 && s[3] == 0 && s[4] == 0xffff && s[5] == 0 {
        return Some(v4_from_low32(s));
    }
    // 64:ff9b::a.b.c.d — NAT64 well-known prefix (RFC 6052): a translator
    // strips it and forwards to the embedded v4. 64:ff9b:1::/48 (RFC 8215
    // local-use) has a translator-defined offset a client cannot know, so it
    // is refused whole in classify_v6 rather than guessed at here.
    if s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0 {
        return Some(v4_from_low32(s));
    }
    // 2002:a.b.c.d::/48 — 6to4 (RFC 3056): the v4 is segments 1 and 2, and a
    // 6to4 relay routes to it. Deprecated (RFC 7526) but still relayed.
    if s[0] == 0x2002 {
        return Some(Ipv4Addr::new(
            (s[1] >> 8) as u8,
            (s[1] & 0xff) as u8,
            (s[2] >> 8) as u8,
            (s[2] & 0xff) as u8,
        ));
    }
    None
}

fn v4_from_low32(s: [u16; 8]) -> Ipv4Addr {
    Ipv4Addr::new(
        (s[6] >> 8) as u8,
        (s[6] & 0xff) as u8,
        (s[7] >> 8) as u8,
        (s[7] & 0xff) as u8,
    )
}

fn classify_v4(ip: Ipv4Addr) -> AddressClass {
    let o = ip.octets();

    // Never-permitted specials first.
    if ip.is_unspecified()          // 0.0.0.0
        || ip.is_link_local()       // 169.254/16 — includes 169.254.169.254
        || ip.is_broadcast()        // 255.255.255.255
        || ip.is_multicast()        // 224/4
        || ip.is_documentation()    // 192.0.2/24, 198.51.100/24, 203.0.113/24
        || o[0] == 0                // 0/8 "this network"
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)   // 192.0.0/24 IETF protocol
        || (o[0] == 198 && (o[1] & 0xfe) == 18)      // 198.18/15 benchmarking
        || o[0] >= 240
    // 240/4 reserved (Class E), catches 255/8
    {
        return AddressClass::Forbidden;
    }

    // Owned-network ranges: blocked by default, allowable for an owned host.
    if ip.is_loopback()             // 127/8
        || ip.is_private()          // 10/8, 172.16/12, 192.168/16
        || is_cgnat_v4(o)           // 100.64/10 carrier-grade NAT
        || (o[0] == 192 && o[1] == 168)
    // (redundant with is_private, explicit)
    {
        return AddressClass::PrivateOrLoopback;
    }

    AddressClass::Public
}

fn is_cgnat_v4(o: [u8; 4]) -> bool {
    o[0] == 100 && (64..=127).contains(&o[1])
}

/// Classify a genuine (non-v4-embedding) v6 address. The v4-in-v6 forms are
/// unwrapped by [`embedded_v4`] before this is reached.
fn classify_v6(ip: Ipv6Addr) -> AddressClass {
    // Owned-network first: loopback and ULA are non-global but are the "a
    // host you own" case, so allow_insecure may reach them.
    if ip.is_loopback()             // ::1
        || is_v6_unique_local(ip)
    // fc00::/7
    {
        return AddressClass::PrivateOrLoopback;
    }

    // Everything below is never permitted, under any policy.
    //
    // The load-bearing line is the last one: default-deny outside global
    // unicast. It makes the explicit checks above it belt-and-braces (each
    // range is also outside 2000::/3), but they document intent and guard
    // against a future edit that widens the global-unicast test.
    if ip.is_multicast()                       // ff00::/8
        || ip.is_unspecified()                 // ::  (routes to localhost)
        || is_v6_link_local(ip)                // fe80::/10
        || is_v6_documentation(ip)             // 2001:db8::/32 + 3fff::/20 doc, 2001:2::/48 bench
        || is_teredo(ip)                       // 2001:0000::/32 (inside 2000::/3)
        || is_v6_discard(ip)                   // 100::/64
        || is_nat64_local_use(ip)              // 64:ff9b:1::/48
        || !is_global_unicast(ip)
    // <-- catches fec0::/10 and every unnamed range
    {
        return AddressClass::Forbidden;
    }

    // Accepted residual (round-2 refute-panel, all ruled LOW / not-real
    // because none reaches an internal host): 6to4 relay anycast
    // 192.88.99.0/24, ORCHID 2001:10::/28, and ISATAP suffixes inside global
    // unicast stay classified by their prefix. They route to public or
    // non-existent space, not to anything internal, so they are left as-is
    // rather than special-cased — enumerating them would be policy theatre.
    AddressClass::Public
}

/// Global unicast `2000::/3` — the only currently-allocated globally routable
/// v6 space. Everything else is special-purpose, reserved, or unallocated;
/// failing closed on it is the whole point of the default-deny.
fn is_global_unicast(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xe000) == 0x2000
}

fn is_v6_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn is_v6_unique_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

/// Non-routable documentation and benchmarking ranges. `2001:db8::/32` is
/// the classic; `3fff::/20` (RFC 9637) is the newer doc range and `2001:2::/48`
/// (RFC 5180) is benchmarking. All three are siblings — blocking one and not
/// the others would be an unexplained gap. (The v4 equivalents are already
/// Forbidden in `classify_v4`.)
fn is_v6_documentation(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    (s[0] == 0x2001 && s[1] == 0x0db8)              // 2001:db8::/32
        || (s[0] == 0x2001 && s[1] == 0x0002 && s[2] == 0) // 2001:2::/48 benchmarking
        || (s[0] == 0x3fff && s[1] <= 0x0fff) // 3fff::/20 documentation
}

/// Teredo `2001:0000::/32` (RFC 4380). The client v4 is the last 32 bits
/// XOR-obfuscated; rather than decode and classify it, the whole (deprecated)
/// prefix is refused — it is not a documentation host.
fn is_teredo(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    s[0] == 0x2001 && s[1] == 0x0000
}

fn is_v6_discard(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    s[0] == 0x0100 && s[1] == 0 && s[2] == 0 && s[3] == 0
}

/// NAT64 local-use prefix `64:ff9b:1::/48` (RFC 8215). Unlike the well-known
/// prefix, its embedded-v4 offset is translator-defined, so the address is
/// refused whole rather than unwrapped.
fn is_nat64_local_use(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 0x0001
}

/// The policy for a fetch, derived from the source config's `allow_insecure`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressPolicy {
    /// When true, `PrivateOrLoopback` is permitted (an owned intranet host).
    /// `Forbidden` is never permitted.
    allow_private: bool,
}

impl AddressPolicy {
    /// The default: only public addresses.
    pub fn public_only() -> Self {
        Self {
            allow_private: false,
        }
    }

    /// For a source config with `allow_insecure: true`.
    pub fn allow_private() -> Self {
        Self {
            allow_private: true,
        }
    }

    pub fn permits(&self, ip: IpAddr) -> bool {
        match classify(ip) {
            AddressClass::Public => true,
            AddressClass::PrivateOrLoopback => self.allow_private,
            AddressClass::Forbidden => false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // panicking on a bad test literal is correct
mod tests {
    use super::*;
    use std::str::FromStr;

    fn class(s: &str) -> AddressClass {
        classify(IpAddr::from_str(s).expect("valid ip in test"))
    }

    #[test]
    fn public_addresses_are_public() {
        for s in [
            "1.1.1.1",
            "8.8.8.8",
            "93.184.216.34",
            "2606:2800:220:1::",
            "2001:4860:4860::8888",
        ] {
            assert_eq!(class(s), AddressClass::Public, "{s}");
        }
    }

    #[test]
    fn loopback_and_private_are_owned_network() {
        for s in [
            "127.0.0.1",
            "127.5.5.5",
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "100.64.0.1",
            "::1",
            "fc00::1",
            "fd12:3456::1",
        ] {
            assert_eq!(class(s), AddressClass::PrivateOrLoopback, "{s}");
        }
    }

    #[test]
    fn the_cloud_metadata_address_is_forbidden() {
        // The single most important line in this file.
        assert_eq!(class("169.254.169.254"), AddressClass::Forbidden);
    }

    #[test]
    fn special_ranges_are_forbidden() {
        for s in [
            "0.0.0.0",
            "169.254.1.1",
            "255.255.255.255",
            "224.0.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "240.0.0.1",
            "192.0.0.1",
            "::",
            "fe80::1",
            "ff02::1",
            "2001:db8::1",
        ] {
            assert_eq!(class(s), AddressClass::Forbidden, "{s}");
        }
    }

    #[test]
    fn v4_mapped_and_compatible_v6_unwrap_to_v4() {
        // The bypass this defends: a v6 spelling of a v4 loopback/metadata.
        assert_eq!(class("::ffff:127.0.0.1"), AddressClass::PrivateOrLoopback);
        assert_eq!(class("::ffff:169.254.169.254"), AddressClass::Forbidden);
        assert_eq!(class("::ffff:8.8.8.8"), AddressClass::Public);
        // Deprecated ::x compatible form.
        assert_eq!(class("::127.0.0.1"), AddressClass::PrivateOrLoopback);
        assert_eq!(class("::a9fe:a9fe"), AddressClass::Forbidden); // ::169.254.169.254
    }

    // ---- regressions from the S1-5 adversarial refute-panel ----------------

    #[test]
    fn nat64_well_known_prefix_unwraps_to_v4() {
        // The confirmed HIGH bypass: 64:ff9b::/96 reaching the metadata IP.
        assert_eq!(class("64:ff9b::a9fe:a9fe"), AddressClass::Forbidden); // 169.254.169.254
        assert_eq!(class("64:ff9b::7f00:1"), AddressClass::PrivateOrLoopback); // 127.0.0.1
        assert_eq!(class("64:ff9b::a00:1"), AddressClass::PrivateOrLoopback); // 10.0.0.1
                                                                              // DNS64 on an IPv6-only network synthesises this for a public v4 host;
                                                                              // it must stay reachable, or the app breaks on NAT64 networks.
        assert_eq!(class("64:ff9b::808:808"), AddressClass::Public); // 8.8.8.8
                                                                     // allow_insecure does not open the translated metadata endpoint.
        let owned = AddressPolicy::allow_private();
        assert!(!owned.permits("64:ff9b::a9fe:a9fe".parse().unwrap()));
    }

    #[test]
    fn nat64_local_use_prefix_is_forbidden_whole() {
        // 64:ff9b:1::/48 has a translator-defined offset; refuse it entirely.
        assert_eq!(class("64:ff9b:1::a9fe:a9fe"), AddressClass::Forbidden);
        assert_eq!(class("64:ff9b:1::"), AddressClass::Forbidden);
    }

    #[test]
    fn ipv4_translated_siit_unwraps_to_v4() {
        // ::ffff:0:a.b.c.d (RFC 2765) — seg[4]=ffff, seg[5]=0.
        assert_eq!(class("::ffff:0:7f00:1"), AddressClass::PrivateOrLoopback); // 127.0.0.1
        assert_eq!(class("::ffff:0:a9fe:a9fe"), AddressClass::Forbidden); // 169.254.169.254
    }

    #[test]
    fn sixtofour_unwraps_to_the_embedded_v4() {
        // 2002:V4::/48 — a 6to4 relay routes to the embedded v4.
        assert_eq!(class("2002:7f00:1::"), AddressClass::PrivateOrLoopback); // 127.0.0.1
        assert_eq!(class("2002:a9fe:a9fe::"), AddressClass::Forbidden); // 169.254.169.254
        assert_eq!(class("2002:808:808::"), AddressClass::Public); // 8.8.8.8
    }

    #[test]
    fn site_local_and_everything_outside_global_unicast_is_forbidden() {
        // The confirmed MEDIUM bypass: fec0::/10 was classified Public.
        for s in ["fec0::1", "feff::abcd", "fed0:1234::5", "fec0:0:0:ffff::1"] {
            assert_eq!(class(s), AddressClass::Forbidden, "{s}");
        }
        // Teredo and other non-global space.
        assert_eq!(class("2001:0000::1"), AddressClass::Forbidden); // Teredo
                                                                    // Doc/benchmark ranges inside 2000::/3 (round-2 consistency fix).
        assert_eq!(class("2001:2::1"), AddressClass::Forbidden); // RFC 5180 benchmarking
        assert_eq!(class("3fff::1"), AddressClass::Forbidden); // RFC 9637 documentation
        assert_eq!(class("3fff:fff::1"), AddressClass::Forbidden); // top of 3fff::/20
                                                                   // Genuine global unicast still passes, including addresses just
                                                                   // outside the doc/benchmark ranges — proving the masks don't over-block.
        assert_eq!(class("2001:4860:4860::8888"), AddressClass::Public);
        assert_eq!(class("2606:2800:220:1::"), AddressClass::Public);
        assert_eq!(class("2001:3::1"), AddressClass::Public); // just above 2001:2::/48
                                                              // 4000::/3 is NOT global unicast (2000::/3 ends at 3fff:) — default-deny
                                                              // forbids it, which is the point of failing closed outside 2000::/3.
        assert_eq!(class("4000::1"), AddressClass::Forbidden);
    }

    #[test]
    fn policy_gates_private_but_never_forbidden() {
        let public_only = AddressPolicy::public_only();
        let owned = AddressPolicy::allow_private();

        let public: IpAddr = "8.8.8.8".parse().unwrap();
        let private: IpAddr = "10.0.0.1".parse().unwrap();
        let metadata: IpAddr = "169.254.169.254".parse().unwrap();

        assert!(public_only.permits(public) && owned.permits(public));
        assert!(!public_only.permits(private) && owned.permits(private));
        // allow_insecure does NOT open the metadata endpoint.
        assert!(!public_only.permits(metadata) && !owned.permits(metadata));
    }

    #[test]
    fn cgnat_boundaries() {
        assert_eq!(class("100.63.255.255"), AddressClass::Public); // just below
        assert_eq!(class("100.64.0.0"), AddressClass::PrivateOrLoopback);
        assert_eq!(class("100.127.255.255"), AddressClass::PrivateOrLoopback);
        assert_eq!(class("100.128.0.0"), AddressClass::Public); // just above
    }
}
