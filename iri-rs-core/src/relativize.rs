//! Relativization — inverse of [`crate::resolve::resolve`].
//!
//! Given an absolute base `B` and absolute target `T`, produce a reference `R`
//! such that `resolve(B, R) == T` (after RFC 3986 §5.2 dot-segment removal).
//!
//! Strategy mirrors the reverse of RFC 3986 §5.2:
//!   * different scheme → emit absolute target
//!   * different authority → emit network-path reference (`//auth/path?q#f`)
//!   * different path → emit `../`-prefixed path-relative reference
//!   * same path, different query → emit `?query[#frag]`
//!   * same path+query, different fragment → emit `#frag`
//!   * identical → empty string

use memchr::memchr;

use crate::parse::Positions;

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

    let base_auth = &base_s[base_p.scheme_end..base_p.authority_end];
    let tgt_auth = &tgt_s[tgt_p.scheme_end..tgt_p.authority_end];
    if base_auth != tgt_auth {
        output.push_str(&tgt_s[tgt_p.scheme_end..]);
        return;
    }

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
        } else {
            output.push_str(tgt_tail);
        }
        return;
    }

    write_relative_path(base_path, tgt_path, output);
    output.push_str(tgt_tail);
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

    let up = base_rest.as_bytes().iter().filter(|&&b| b == b'/').count();
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
            let first = tgt_rest.split('/').next().unwrap_or("");
            if first.contains(':') {
                output.push_str("./");
            }
        }
        output.push_str(tgt_rest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::find_iri_positions;
    use crate::resolve::resolve;

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
}
