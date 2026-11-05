//! Relativization — inverse of [`crate::resolve::resolve`].
//!
//! Given an absolute base `B` and absolute target `T`, produce a reference `R`
//! such that `resolve(B, R) == resolve(B, T)`: `T` itself, after the RFC 3986
//! §5.2.2 dot-segment removal resolution applies to it.
//!
//! Strategy mirrors the reverse of RFC 3986 §5.2:
//!   * different scheme → emit absolute target
//!   * authority on one side only → emit absolute target
//!   * different authority → emit network-path reference (`//auth/path?q#f`)
//!   * different path → emit `../`-prefixed path-relative reference
//!   * same path, different query → emit `?query[#frag]`
//!   * same path, target has no query → restate the last segment
//!   * same path+query, different fragment → emit `#frag`
//!   * identical → empty string
//!
//! Those rules assume a base whose path holds no dot segments and resolves as
//! written; a rootless base path climbed by `..`, for one, resolves to an
//! absolute path (§5.2.4). So a reference built from a shared authority is
//! resolved once to check it, and replaced by the target minus its scheme,
//! which always resolves back, when it does not.

use memchr::{memchr, memrchr};

use crate::{
    components::path_is_normalized,
    parse::{Positions, find_iri_ref_positions},
    resolve::resolve,
};

/// Write a reference for `target` relative to `base` into `output`.
pub fn relativize(base: (&str, Positions), target: (&str, Positions), output: &mut String) {
    let (base_s, base_p) = base;
    let (tgt_s, tgt_p) = target;

    let base_scheme = &base_s[..base_p.scheme_end];
    let tgt_scheme = &tgt_s[..tgt_p.scheme_end];
    if base_scheme != tgt_scheme {
        output.push_str(tgt_s);
        return;
    }

    // A reference may only omit what the base can supply. When exactly one side
    // has an authority there is no shorter form: dropping the scheme would
    // leave a path-only reference, and resolution would hand it the base's
    // authority (RFC 3986 §5.2.2) — the wrong one either way.
    let base_has_authority = base_p.authority_end > base_p.scheme_end;
    let tgt_has_authority = tgt_p.authority_end > tgt_p.scheme_end;
    if base_has_authority != tgt_has_authority {
        output.push_str(tgt_s);
        return;
    }

    let base_auth = &base_s[base_p.scheme_end..base_p.authority_end];
    let tgt_auth = &tgt_s[tgt_p.scheme_end..tgt_p.authority_end];
    if base_auth != tgt_auth {
        output.push_str(&tgt_s[tgt_p.scheme_end..]);
        return;
    }

    let start = output.len();
    write_within_authority(base, target, output);
    if !resolves_to_target(base, target, &output[start..]) {
        // With the scheme, base and target share everything a reference could
        // omit. A network-path or absolute reference takes nothing from the
        // base but its scheme, and resolution removes the same dot segments
        // from it as from the target.
        output.truncate(start);
        output.push_str(if tgt_has_authority { &tgt_s[tgt_p.scheme_end..] } else { tgt_s });
    }
}

/// Whether `reference` resolves against `base` to what `target` resolves to.
fn resolves_to_target(base: (&str, Positions), target: (&str, Positions), reference: &str) -> bool {
    let mut got = String::with_capacity(base.0.len() + reference.len());
    resolve(base, (reference, find_iri_ref_positions(reference)), &mut got);
    let (tgt_s, tgt_p) = target;
    if path_is_normalized(&tgt_s[tgt_p.authority_end..tgt_p.path_end]) {
        // Without dot segments the target resolves to itself.
        return got == tgt_s;
    }
    let mut want = String::with_capacity(tgt_s.len());
    resolve(base, target, &mut want);
    got == want
}

/// The reference for a target sharing the base's scheme and authority.
fn write_within_authority(base: (&str, Positions), target: (&str, Positions), output: &mut String) {
    let (base_s, base_p) = base;
    let (tgt_s, tgt_p) = target;

    let base_path = &base_s[base_p.authority_end..base_p.path_end];
    let tgt_path = &tgt_s[tgt_p.authority_end..tgt_p.path_end];
    let tgt_tail = &tgt_s[tgt_p.path_end..];

    if base_path == tgt_path {
        let base_query_present = base_p.path_end < base_p.query_end;
        let tgt_query_present = tgt_p.path_end < tgt_p.query_end;
        let base_query = if base_query_present {
            Some(&base_s[base_p.path_end + 1..base_p.query_end])
        } else {
            None
        };
        let tgt_query = if tgt_query_present {
            Some(&tgt_s[tgt_p.path_end + 1..tgt_p.query_end])
        } else {
            None
        };
        if base_query == tgt_query {
            let tgt_frag = if tgt_p.query_end < tgt_s.len() {
                Some(&tgt_s[tgt_p.query_end + 1..])
            } else {
                None
            };
            if let Some(f) = tgt_frag {
                output.push('#');
                output.push_str(f);
            }
        } else if tgt_query.is_none() {
            // RFC 3986 §5.2.2 only lets a reference override the base's query
            // when its path is non-empty, so dropping a query the base carries
            // means restating the path the two already share.
            if !write_same_path(tgt_path, output) {
                // Rejected by the round-trip check, which falls back.
                output.push_str(tgt_s);
                return;
            }
            output.push_str(tgt_tail);
        } else {
            output.push_str(tgt_tail);
        }
        return;
    }

    write_relative_path(base_path, tgt_path, output);
    output.push_str(tgt_tail);
}

/// Writes the shortest non-empty relative path that resolves back to `path`
/// against a base sharing that same path — its last segment, or `./` when it
/// names a directory.
///
/// Returns `false` when no such path exists and the caller has to fall back to
/// the target in full.
fn write_same_path(path: &str, output: &mut String) -> bool {
    if path.is_empty() {
        // Every non-empty reference path would resolve somewhere else, and the
        // empty one is exactly what the caller cannot use.
        return false;
    }
    let last_segment = match memrchr(b'/', path.as_bytes()) {
        Some(slash) => &path[slash + 1..],
        None => path,
    };
    match last_segment {
        "" => output.push_str("./"),
        // Dot segments do not survive resolution, so a path ending in one
        // cannot be reproduced by restating it.
        "." | ".." => return false,
        // A first segment holding a `:` would be read as a scheme
        // (RFC 3986 §4.2); the `./` keeps it a path.
        segment if memchr(b':', segment.as_bytes()).is_some() => {
            output.push_str("./");
            output.push_str(segment);
        }
        segment => output.push_str(segment),
    }
    true
}

fn write_relative_path(base_path: &str, tgt_path: &str, output: &mut String) {
    let base_abs = base_path.starts_with('/');
    let tgt_abs = tgt_path.starts_with('/');
    if base_abs != tgt_abs {
        output.push_str(tgt_path);
        return;
    }

    let mut base_rest = base_path;
    let mut tgt_rest = tgt_path;
    loop {
        let bs = memchr(b'/', base_rest.as_bytes());
        let ts = memchr(b'/', tgt_rest.as_bytes());
        match (bs, ts) {
            (Some(bi), Some(ti)) if bi == ti && base_rest[..bi] == tgt_rest[..ti] => {
                base_rest = &base_rest[bi + 1..];
                tgt_rest = &tgt_rest[ti + 1..];
            }
            _ => break,
        }
    }

    let up = memchr::memchr_iter(b'/', base_rest.as_bytes()).count();
    if up == 0 && tgt_rest.is_empty() {
        if !base_rest.is_empty() {
            output.push_str("./");
        }
        return;
    }
    for _ in 0..up {
        output.push_str("../");
    }
    if !tgt_rest.is_empty() {
        if up == 0 {
            // An empty first segment would make the path absolute, and one
            // holding a `:` would read as a scheme (RFC 3986 §4.2).
            let first = tgt_rest.split('/').next().unwrap_or("");
            if first.is_empty() || first.contains(':') {
                output.push_str("./");
            }
        }
        output.push_str(tgt_rest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse::find_iri_positions, resolve::resolve};

    fn rel(base: &str, tgt: &str) -> String {
        let bp = find_iri_positions(base);
        let tp = find_iri_positions(tgt);
        let mut out = String::new();
        relativize((base, bp), (tgt, tp), &mut out);
        out
    }

    fn roundtrip(base: &str, tgt: &str) {
        let r = rel(base, tgt);
        let bp = find_iri_positions(base);
        let rp = crate::parse::find_iri_ref_positions(&r);
        let mut resolved = String::new();
        resolve((base, bp), (&r, rp), &mut resolved);
        assert_eq!(resolved, tgt, "base={base:?} tgt={tgt:?} rel={r:?}");
    }

    #[test]
    fn identical() {
        assert_eq!(rel("http://a/b/c", "http://a/b/c"), "");
        roundtrip("http://a/b/c", "http://a/b/c");
    }

    #[test]
    fn fragment_only() {
        assert_eq!(rel("http://a/b/c", "http://a/b/c#x"), "#x");
        roundtrip("http://a/b/c", "http://a/b/c#x");
    }

    #[test]
    fn query_change() {
        assert_eq!(rel("http://a/b/c?q1", "http://a/b/c?q2"), "?q2");
        roundtrip("http://a/b/c?q1", "http://a/b/c?q2");
    }

    #[test]
    fn sibling_path() {
        assert_eq!(rel("http://a/b/c/d", "http://a/b/c/e"), "e");
        roundtrip("http://a/b/c/d", "http://a/b/c/e");
    }

    #[test]
    fn child_path() {
        assert_eq!(rel("http://a/b/c", "http://a/b/c/d"), "c/d");
        roundtrip("http://a/b/c", "http://a/b/c/d");
    }

    #[test]
    fn parent_path() {
        assert_eq!(rel("http://a/b/c/d", "http://a/b/x"), "../x");
        roundtrip("http://a/b/c/d", "http://a/b/x");
    }

    #[test]
    fn deep_parent() {
        assert_eq!(rel("http://a/b/c/d/e", "http://a/x"), "../../../x");
        roundtrip("http://a/b/c/d/e", "http://a/x");
    }

    #[test]
    fn different_authority() {
        assert_eq!(rel("http://a/x", "http://b/y"), "//b/y");
        roundtrip("http://a/x", "http://b/y");
    }

    #[test]
    fn different_scheme() {
        assert_eq!(rel("http://a/x", "https://a/x"), "https://a/x");
        roundtrip("http://a/x", "https://a/x");
    }

    #[test]
    fn target_has_query_and_frag() {
        assert_eq!(rel("http://a/b/c", "http://a/b/d?q#f"), "d?q#f");
        roundtrip("http://a/b/c", "http://a/b/d?q#f");
    }

    #[test]
    fn first_segment_with_colon_needs_dot_slash() {
        // "foo:bar" alone would parse as scheme; emit "./foo:bar" instead.
        let r = rel("http://a/b/c", "http://a/b/foo:bar");
        assert_eq!(r, "./foo:bar");
        roundtrip("http://a/b/c", "http://a/b/foo:bar");
    }

    #[test]
    fn empty_base_path() {
        // base path is "", target path "/x" → different absolute-ness.
        // Edge case: just emit absolute path.
        assert_eq!(rel("http://a", "http://a/x"), "/x");
        roundtrip("http://a", "http://a/x");
    }

    /// Vectors from the upstream `iref` suite, which in turn come from the
    /// JSON-LD API test base IRIs.
    #[test]
    fn json_ld_base_relativizes() {
        const BASE: &str = "https://w3c.github.io/json-ld-api/tests/compact/0066-in.jsonld";
        for (target, expected) in [
            ("https://w3c.github.io/json-ld-api/tests/compact/link", "link"),
            (
                "https://w3c.github.io/json-ld-api/tests/compact/0066-in.jsonld#fragment-works",
                "#fragment-works",
            ),
            ("https://w3c.github.io/json-ld-api/tests/compact/0066-in.jsonld?query=works", "?query=works"),
            ("https://w3c.github.io/json-ld-api/tests/", "../"),
            ("https://w3c.github.io/json-ld-api/", "../../"),
            ("https://w3c.github.io/json-ld-api/parent", "../../parent"),
            ("https://w3c.github.io/json-ld-api/parent#fragment", "../../parent#fragment"),
            ("https://w3c.github.io/parent-parent-eq-root", "../../../parent-parent-eq-root"),
            ("http://example.org/scheme-relative", "http://example.org/scheme-relative"),
            ("https://w3c.github.io/json-ld-api/tests/compact/0066-in.jsonld", ""),
        ] {
            assert_eq!(rel(BASE, target), expected, "target `{target}`");
            roundtrip(BASE, target);
        }
    }

    #[test]
    fn relativizes_against_an_empty_base_path() {
        for (target, expected) in [
            ("http://a/", "/"),
            ("http://a/g", "/g"),
            ("http://a/g/h", "/g/h"),
            ("http://a?q", "?q"),
            ("http://a#f", "#f"),
            ("http://a?q#f", "?q#f"),
        ] {
            assert_eq!(rel("http://a", target), expected, "target `{target}`");
            roundtrip("http://a", target);
        }
    }

    #[test]
    fn relativizes_against_a_root_base_path() {
        for (target, expected) in [
            ("http://a/", ""),
            ("http://a/g", "g"),
            ("http://a/g/h", "g/h"),
            ("http://a/?q", "?q"),
            ("http://a/#f", "#f"),
        ] {
            assert_eq!(rel("http://a/", target), expected, "target `{target}`");
            roundtrip("http://a/", target);
        }
    }

    /// A differing authority still shares the scheme, so a network-path
    /// reference is enough — shorter than the absolute IRI upstream emits, and
    /// it resolves back to the same target.
    #[test]
    fn different_authority_uses_a_network_path_reference() {
        for (target, expected) in [("http://b/path", "//b/path"), ("http://b/other", "//b/other")] {
            assert_eq!(rel("http://a/path", target), expected, "target `{target}`");
            roundtrip("http://a/path", target);
        }
    }

    #[test]
    fn mismatched_scheme_keeps_the_target() {
        for target in ["https://a/path", "ftp://a/path"] {
            assert_eq!(rel("http://a/path", target), target, "target `{target}`");
            roundtrip("http://a/path", target);
        }
    }

    /// An authority on one side only cannot be relativized: any reference short
    /// enough to drop the scheme would be handed the base's authority on the
    /// way back, so the target has to be emitted whole.
    #[test]
    fn authority_on_one_side_only_keeps_the_target() {
        assert_eq!(rel("http://a/path", "http:/path"), "http:/path");
        roundtrip("http://a/path", "http:/path");

        assert_eq!(rel("http:/path", "http://a/path"), "http://a/path");
        roundtrip("http:/path", "http://a/path");
    }

    /// RFC 3986 §5.2.2 only lets a reference override the base's query when its
    /// path is non-empty, so a target that drops the query has to restate the
    /// path both already share.
    #[test]
    fn target_without_a_query_restates_the_shared_path() {
        for (base, target, expected) in [
            ("http://a/b/c?q1", "http://a/b/c", "c"),
            ("http://a/b/c?q1", "http://a/b/c#f", "c#f"),
            ("http://a/b/?q1", "http://a/b/", "./"),
            ("http://a/?q1", "http://a/", "./"),
            ("http://a/b/foo:bar?q", "http://a/b/foo:bar", "./foo:bar"),
            ("urn:foo?q", "urn:foo", "foo"),
        ] {
            assert_eq!(rel(base, target), expected, "base `{base}` target `{target}`");
            roundtrip(base, target);
        }
    }

    #[test]
    fn target_without_a_query_falls_back_when_the_path_cannot_be_restated() {
        // An empty path has no last segment to repeat, and every non-empty
        // reference path would resolve somewhere else.
        assert_eq!(rel("http://a?q", "http://a"), "http://a");
        roundtrip("http://a?q", "http://a");

        // A path ending in a dot segment is not reachable through resolution at
        // all — `remove_dot_segments` would rewrite it — so no roundtrip is
        // possible and the target is returned verbatim.
        assert_eq!(rel("http://a/b/.?q", "http://a/b/."), "http://a/b/.");
        assert_eq!(rel("http://a/b/..?q", "http://a/b/.."), "http://a/b/..");
    }

    /// Shapes the path rules alone get wrong, and what replaces them.
    #[test]
    fn falls_back_where_a_shorter_reference_would_resolve_elsewhere() {
        for (base, target, expected) in [
            // `//x` would read as an authority.
            ("http://h", "http://h//x", "//h//x"),
            // An empty path resolves to the base's path.
            ("http://h/a", "http://h", "//h"),
            ("http://h/a", "http://h?q", "//h?q"),
            // `..` climbing a rootless path resolves to an absolute one.
            ("foo:a/b", "foo:c", "foo:c"),
            // The base's dot segments are not removed by resolving against it.
            ("http://h/a/./b", "http://h/a/c", "//h/a/c"),
        ] {
            assert_eq!(rel(base, target), expected, "base `{base}` target `{target}`");
            roundtrip(base, target);
        }
    }

    /// An empty segment after the shared directory keeps a `./`, or the rest
    /// would be an absolute path.
    #[test]
    fn empty_segment_after_the_shared_directory_keeps_dot_slash() {
        assert_eq!(rel("http://h/a/b", "http://h/a//x"), ".//x");
        roundtrip("http://h/a/b", "http://h/a//x");
        assert_eq!(rel("http://h/a/b/c", "http://h/a//x"), "..//x");
        roundtrip("http://h/a/b/c", "http://h/a//x");
    }

    /// Random IRIs over a small alphabet of the shapes that are easy to get
    /// wrong: empty segments, dot segments, `:` in a segment, rootless paths,
    /// empty and missing components.
    fn random_iri(next: &mut impl FnMut() -> usize) -> String {
        const SCHEMES: &[&str] = &["http:", "foo:"];
        const AUTHORITIES: &[&str] = &["", "//h", "//h", "//", "//g"];
        const SEGMENTS: &[&str] = &["", "a", "a", "b", ".", "..", "a:b", "%2E"];
        let mut s = String::from(SCHEMES[next() % SCHEMES.len()]);
        let authority = AUTHORITIES[next() % AUTHORITIES.len()];
        s.push_str(authority);
        let segments = next() % 5;
        let rooted = !authority.is_empty() || next() % 2 == 0;
        for i in 0..segments {
            if i > 0 || rooted {
                s.push('/');
            }
            s.push_str(SEGMENTS[next() % SEGMENTS.len()]);
        }
        if next() % 3 == 0 {
            s.push_str(["?", "?q", "?r"][next() % 3]);
        }
        if next() % 3 == 0 {
            s.push_str(["#", "#f"][next() % 2]);
        }
        s
    }

    /// The reference resolves back to whatever the target itself resolves
    /// to against the base, which is the target with its dot segments removed.
    #[test]
    fn round_trips_on_random_pairs() {
        use crate::parse::find_iri_ref_positions;
        let mut state: u32 = 0x5eed_1234;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as usize
        };
        let resolve_against = |base: &str, r: &str| {
            let mut out = String::new();
            resolve((base, find_iri_positions(base)), (r, find_iri_ref_positions(r)), &mut out);
            out
        };
        let mut failures = Vec::new();
        for _ in 0..50_000 {
            let (base, target) = (random_iri(&mut next), random_iri(&mut next));
            if crate::validate::validate_iri_ref(&base, find_iri_ref_positions(&base), true).is_err()
                || crate::validate::validate_iri_ref(&target, find_iri_ref_positions(&target), true).is_err()
            {
                continue;
            }
            let r = rel(&base, &target);
            let (got, want) = (resolve_against(&base, &r), resolve_against(&base, &target));
            if got != want {
                failures.push(format!("base={base:?} target={target:?} rel={r:?} -> {got:?}, want {want:?}"));
            }
        }
        assert!(
            failures.is_empty(),
            "{} failures, e.g.\n{}",
            failures.len(),
            failures[..failures.len().min(40)].join("\n")
        );
    }

    #[test]
    fn round_trips_across_bases_and_targets() {
        for base in ["http://a/b/c/d", "http://a/b/c/", "http://a/", "http://a"] {
            for target in [
                "http://a/",
                "http://a/g",
                "http://a/b/g",
                "http://a/b/c/g",
                "http://a/b/c/g/",
                "http://a/b/c/?q",
                "http://a/b/c/#f",
                "http://a/b/c/g?q#f",
            ] {
                roundtrip(base, target);
            }
        }
    }
}
