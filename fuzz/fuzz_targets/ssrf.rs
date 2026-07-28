//! The SSRF address classifier must be total and self-consistent.
//!
//! `classify` is pure address arithmetic, so the fuzzer's job is less "make
//! it panic" (there is little to panic on) and more "find an address that
//! two spellings classify differently" — the v4-mapped-v6 bypass class. The
//! invariant asserted: a v4 address and every v6 spelling of it classify
//! identically, so no v6 costume changes a v4 verdict.

#![no_main]

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use libfuzzer_sys::fuzz_target;
use tome_core::ssrf::{classify, AddressClass, AddressPolicy};

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    let v4 = Ipv4Addr::new(data[0], data[1], data[2], data[3]);
    let direct = classify(IpAddr::V4(v4));
    let o = v4.octets();

    // Every v6 spelling that embeds this v4 — the ones a translator or relay
    // would forward to it — must classify identically. This is the bypass
    // class the S1-5 refute-panel exploited (NAT64 reaching 169.254.169.254).
    let seg = |a: u8, b: u8| ((a as u16) << 8) | b as u16;
    let embeddings = [
        Ipv6Addr::from(v4.to_ipv6_mapped()),                    // ::ffff:v4
        // ::ffff:0:v4 IPv4-translated (SIIT).
        Ipv6Addr::new(0, 0, 0, 0, 0xffff, 0, seg(o[0], o[1]), seg(o[2], o[3])),
        // 64:ff9b::v4 NAT64 well-known prefix.
        Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, seg(o[0], o[1]), seg(o[2], o[3])),
        // 2002:v4::/48 6to4.
        Ipv6Addr::new(0x2002, seg(o[0], o[1]), seg(o[2], o[3]), 0, 0, 0, 0, 0),
    ];
    for embedded in embeddings {
        assert_eq!(
            classify(IpAddr::V6(embedded)),
            direct,
            "embedding {embedded} of {v4} disagreed"
        );
    }

    // Policy is monotone: allow_private is a superset of public_only, and
    // nothing makes a Forbidden address permitted.
    let public_only = AddressPolicy::public_only();
    let owned = AddressPolicy::allow_private();
    let ip = IpAddr::V4(v4);
    if public_only.permits(ip) {
        assert!(owned.permits(ip), "owned policy must be a superset");
    }
    if direct == AddressClass::Forbidden {
        assert!(!public_only.permits(ip) && !owned.permits(ip), "Forbidden is never permitted");
    }

    // Exercise a v6 address built from the remaining bytes, for totality.
    if data.len() >= 20 {
        let mut seg = [0u8; 16];
        seg.copy_from_slice(&data[4..20]);
        let v6 = Ipv6Addr::from(seg);
        let _ = classify(IpAddr::V6(v6));
        let _ = owned.permits(IpAddr::V6(v6));
    }
});
