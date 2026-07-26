//! RFC 3986/3987 §6.2.2 syntax-based normalization — streaming comparators + hash.

use std::{cmp::Ordering, hash::Hasher};

use pct::PctStr;

use crate::{
    components::{normalize_path, path_is_normalized},
    parse::Positions,
};

pub fn iri_eq(a: &str, pa: Positions, b: &str, pb: Positions) -> bool {
    if a == b {
        return true;
    }
    let sa = if pa.scheme_end > 0 { Some(&a[..pa.scheme_end - 1]) } else { None };
    let sb = if pb.scheme_end > 0 { Some(&b[..pb.scheme_end - 1]) } else { None };
    if sa.map(str::len) != sb.map(str::len) {
        return false;
    }
    match (sa, sb) {
        (Some(x), Some(y)) => {
            if !x.eq_ignore_ascii_case(y) {
                return false;
            }
        }
        (None, None) => {}
        _ => return false,
    }
    let aa = if pa.authority_end > pa.scheme_end + 2 {
        Some(&a[pa.scheme_end + 2..pa.authority_end])
    } else if pa.authority_end == pa.scheme_end {
        None
    } else {
        Some("")
    };
    let ab = if pb.authority_end > pb.scheme_end + 2 {
        Some(&b[pb.scheme_end + 2..pb.authority_end])
    } else if pb.authority_end == pb.scheme_end {
        None
    } else {
        Some("")
    };
    match (aa, ab) {
        (Some(x), Some(y)) => {
            if !authority_eq(x, y) {
                return false;
            }
        }
        (None, None) => {}
        _ => return false,
    }
    let pa_path = &a[pa.authority_end..pa.path_end];
    let pb_path = &b[pb.authority_end..pb.path_end];
    if !path_eq_normalized(pa_path, pb_path) {
        return false;
    }
    let qa = if pa.query_end > pa.path_end {
        Some(&a[pa.path_end + 1..pa.query_end])
    } else {
        None
    };
    let qb = if pb.query_end > pb.path_end {
        Some(&b[pb.path_end + 1..pb.query_end])
    } else {
        None
    };
    if !opt_pct_unreserved_eq(qa, qb) {
        return false;
    }
    let fa = if a.len() > pa.query_end { Some(&a[pa.query_end + 1..]) } else { None };
    let fb = if b.len() > pb.query_end { Some(&b[pb.query_end + 1..]) } else { None };
    opt_pct_unreserved_eq(fa, fb)
}

pub fn iri_cmp(a: &str, pa: Positions, b: &str, pb: Positions) -> Ordering {
    let sa = if pa.scheme_end > 0 { &a[..pa.scheme_end - 1] } else { "" };
    let sb = if pb.scheme_end > 0 { &b[..pb.scheme_end - 1] } else { "" };
    match sa.bytes().map(|c| c.to_ascii_lowercase()).cmp(sb.bytes().map(|c| c.to_ascii_lowercase())) {
        Ordering::Equal => {}
        o => return o,
    }
    a[pa.scheme_end..].cmp(&b[pb.scheme_end..])
}

pub fn authority_eq(a: &str, b: &str) -> bool {
    let (ui_a, rest_a) = split_user_info(a);
    let (ui_b, rest_b) = split_user_info(b);
    if ui_a != ui_b {
        return false;
    }
    let (host_a, port_a) = split_host_port(rest_a);
    let (host_b, port_b) = split_host_port(rest_b);
    if !host_a.eq_ignore_ascii_case(host_b) {
        return false;
    }
    port_a == port_b
}

fn split_user_info(s: &str) -> (Option<&str>, &str) {
    match memchr::memchr(b'@', s.as_bytes()) {
        Some(i) => (Some(&s[..i]), &s[i + 1..]),
        None => (None, s),
    }
}

fn split_host_port(s: &str) -> (&str, Option<&str>) {
    if let Some(rest) = s.strip_prefix('[') {
        if let Some(end) = memchr::memchr(b']', rest.as_bytes()) {
            let host_end = end + 2;
            if host_end <= s.len() {
                let host = &s[..host_end];
                let tail = &s[host_end..];
                return if let Some(p) = tail.strip_prefix(':') {
                    (host, Some(p))
                } else {
                    (host, None)
                };
            }
        }
    }
    match memchr::memchr(b':', s.as_bytes()) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    }
}

pub fn path_eq_normalized(a: &str, b: &str) -> bool {
    let an = normalize_path(a);
    let bn = normalize_path(b);
    pct_unreserved_eq(&an, &bn)
}

fn opt_pct_unreserved_eq(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => pct_unreserved_eq(x, y),
        (None, None) => true,
        _ => false,
    }
}

pub fn pct_unreserved_eq(a: &str, b: &str) -> bool {
    if memchr::memchr(b'%', a.as_bytes()).is_none() && memchr::memchr(b'%', b.as_bytes()).is_none() {
        return a.as_bytes() == b.as_bytes();
    }
    // SAFETY: callers pass validated substrings of an already-parsed IRI, so
    // any `%` is followed by two hex digits.
    let pa = unsafe { PctStr::new_unchecked(a) };
    let pb = unsafe { PctStr::new_unchecked(b) };
    pa.eq_rfc3986(pb)
}

pub fn normalized_hash<H: Hasher>(iri: &str, p: Positions, state: &mut H) {
    if p.scheme_end > 0 {
        hash_lower_ascii(&iri.as_bytes()[..p.scheme_end - 1], state);
        state.write(b":");
    }
    if p.authority_end > p.scheme_end {
        state.write(b"//");
        let auth = &iri[p.scheme_end + 2..p.authority_end];
        let (ui, rest) = split_user_info(auth);
        if let Some(ui) = ui {
            state.write(ui.as_bytes());
            state.write(b"@");
        }
        let (host, port) = split_host_port(rest);
        hash_lower_ascii(host.as_bytes(), state);
        if let Some(port) = port {
            state.write(b":");
            state.write(port.as_bytes());
        }
    }
    let path = &iri[p.authority_end..p.path_end];
    if path_is_normalized(path) {
        hash_pct_unreserved(path, state);
    } else {
        let np = normalize_path(path);
        hash_pct_unreserved(&np, state);
    }
    if p.query_end > p.path_end {
        state.write(b"?");
        hash_pct_unreserved(&iri[p.path_end + 1..p.query_end], state);
    }
    if iri.len() > p.query_end {
        state.write(b"#");
        hash_pct_unreserved(&iri[p.query_end + 1..], state);
    }
}

fn hash_lower_ascii<H: Hasher>(bytes: &[u8], state: &mut H) {
    let mut buf = [0u8; 128];
    for chunk in bytes.chunks(128) {
        let n = chunk.len();
        buf[..n].copy_from_slice(chunk);
        buf[..n].make_ascii_lowercase();
        state.write(&buf[..n]);
    }
}

fn hash_pct_unreserved<H: Hasher>(s: &str, state: &mut H) {
    if memchr::memchr(b'%', s.as_bytes()).is_none() {
        state.write(s.as_bytes());
        return;
    }
    // SAFETY: input is a validated substring of a parsed IRI.
    let ps = unsafe { PctStr::new_unchecked(s) };
    ps.hash_rfc3986(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{find_iri_positions, find_iri_ref_positions};

    fn eq(a: &str, b: &str) -> bool {
        let pa = find_iri_ref_positions(a);
        let pb = find_iri_ref_positions(b);
        iri_eq(a, pa, b, pb)
    }

    #[test]
    fn byte_eq() {
        assert!(eq("http://a/b", "http://a/b"));
    }

    #[test]
    fn scheme_case_insensitive() {
        assert!(eq("HTTP://a/b", "http://a/b"));
    }

    #[test]
    fn host_case_insensitive() {
        assert!(eq("http://A.com/b", "http://a.com/b"));
    }

    #[test]
    fn pct_unreserved_decoded() {
        assert!(eq("http://a/%7Eb", "http://a/~b"));
    }

    #[test]
    fn pct_hex_case_insensitive() {
        assert!(eq("http://a/%2f", "http://a/%2F"));
    }

    #[test]
    fn path_dot_normalized() {
        let p = find_iri_positions("http://a/b/../c");
        let q = find_iri_positions("http://a/c");
        assert!(iri_eq("http://a/b/../c", p, "http://a/c", q));
    }

    #[test]
    fn different_scheme_ne() {
        assert!(!eq("http://a", "https://a"));
    }

    fn hash_of(s: &str) -> u64 {
        use std::hash::{DefaultHasher, Hasher};
        let mut hasher = DefaultHasher::new();
        normalized_hash(s, find_iri_ref_positions(s), &mut hasher);
        hasher.finish()
    }

    /// A `..` a relative reference cannot resolve away still moves the target
    /// it resolves to, so it may not be normalized out of the comparison.
    #[test]
    fn relative_references_climbing_past_their_root_are_distinct() {
        for (a, b) in [("../a", "a"), ("..", ""), ("a/../..", ""), ("../..", ".."), ("../a", "./a")] {
            assert!(!eq(a, b), "`{a}` should not equal `{b}`");
        }
    }

    #[test]
    fn equal_relative_references_are_recognised() {
        for (a, b) in [("../..", "../../"), ("./a", "a"), ("a/../b", "b"), ("../a/..", "../"), ("x/./y", "x/y")] {
            assert!(eq(a, b), "`{a}` should equal `{b}`");
        }
    }

    /// An empty segment is a path component of its own: under an authority
    /// `//a` and `/a` name different resources.
    #[test]
    fn empty_path_segments_are_significant() {
        assert!(!eq("http://host//a", "http://host/a"));
        assert!(eq("http://host/b/..//a", "http://host//a"));
    }

    #[test]
    fn hash_agrees_with_eq() {
        for (a, b) in [
            ("http://a/b/../c", "http://a/c"),
            ("../..", "../../"),
            ("./a", "a"),
            ("a/../b", "b"),
            ("http://host/b/..//a", "http://host//a"),
        ] {
            assert!(eq(a, b), "`{a}` should equal `{b}`");
            assert_eq!(hash_of(a), hash_of(b), "`{a}` and `{b}` hash differently");
        }
    }

    /// `normalized_hash` skips normalizing when the path is already normal.
    /// Claiming that wrongly would hash an unnormalized path and drop equal
    /// IRIs into different buckets, so the shortcut may be conservative
    /// (`./a:b` is already normal but is not recognised as such) yet never
    /// optimistic.
    #[test]
    fn hash_shortcut_is_never_optimistic() {
        for path in [
            "", "/", "/a/b", "a:b", "./a:b", "a/b:c", "/a:b", "a/../b", "..", "/.//", "mailto@x", "a//b", "/a/./b",
        ] {
            if path_is_normalized(path) {
                assert_eq!(normalize_path(path), path, "`{path}` was claimed normal but normalization changes it");
            }
        }
    }

    #[test]
    fn paths_needing_a_colon_guard_still_hash_together() {
        assert!(eq("urn:a:b", "urn:./a:b"));
        assert_eq!(hash_of("urn:a:b"), hash_of("urn:./a:b"));
    }

    #[test]
    fn distinct_references_hash_apart() {
        for (a, b) in [("../a", "a"), ("..", ""), ("http://host//a", "http://host/a")] {
            assert!(!eq(a, b), "`{a}` should not equal `{b}`");
            assert_ne!(hash_of(a), hash_of(b), "`{a}` and `{b}` hash alike");
        }
    }
}
