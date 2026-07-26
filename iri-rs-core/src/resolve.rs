//! Relative reference resolution — oxiri port. Writes directly into output buffer.
use memchr::{memchr, memmem, memrchr};

use crate::parse::Positions;

/// Resolve `relative` against `base`, appending result to `output_buffer`.
pub fn resolve(base: (&str, Positions), relative: (&str, Positions), output_buffer: &mut String) -> Positions {
    let (base_iri, base_p) = base;
    let (rel_iri, rel_p) = relative;

    // RFC 3986 §5.2.2, absolute-reference branch: `T.path =
    // remove_dot_segments(R.path)`, so the reference cannot simply be copied
    // verbatim when its path carries dot segments.
    if rel_p.scheme_end != 0 {
        output_buffer.reserve_exact(rel_iri.len());
        let path = &rel_iri[rel_p.authority_end..rel_p.path_end];
        if !path_has_dot_segments(path) {
            output_buffer.push_str(rel_iri);
            return rel_p;
        }
        output_buffer.push_str(&rel_iri[..rel_p.authority_end]);
        write_path_without_dot_segments_to(path, output_buffer, rel_p.authority_end, false);
        let path_end = output_buffer.len();
        output_buffer.push_str(&rel_iri[rel_p.path_end..]);
        return Positions {
            scheme_end: rel_p.scheme_end,
            authority_end: rel_p.authority_end,
            path_end,
            query_end: path_end + (rel_p.query_end - rel_p.path_end),
        };
    }
    // RFC 3986 §5.2.2, network-path branch: the base contributes only its
    // scheme, but `T.path = remove_dot_segments(R.path)` applies just the same.
    if rel_p.authority_end > 0 {
        output_buffer.reserve_exact(base_p.scheme_end + rel_iri.len());
        output_buffer.push_str(&base_iri[..base_p.scheme_end]);
        let path = &rel_iri[rel_p.authority_end..rel_p.path_end];
        if !path_has_dot_segments(path) {
            output_buffer.push_str(rel_iri);
            return Positions {
                scheme_end: base_p.scheme_end,
                authority_end: base_p.scheme_end + rel_p.authority_end,
                path_end: base_p.scheme_end + rel_p.path_end,
                query_end: base_p.scheme_end + rel_p.query_end,
            };
        }
        output_buffer.push_str(&rel_iri[..rel_p.authority_end]);
        let authority_end = base_p.scheme_end + rel_p.authority_end;
        write_path_without_dot_segments_to(path, output_buffer, authority_end, false);
        let path_end = output_buffer.len();
        output_buffer.push_str(&rel_iri[rel_p.path_end..]);
        return Positions {
            scheme_end: base_p.scheme_end,
            authority_end,
            path_end,
            query_end: path_end + (rel_p.query_end - rel_p.path_end),
        };
    }
    if rel_p.path_end > 0 {
        if rel_iri.starts_with('/') {
            output_buffer.reserve_exact(base_p.authority_end + rel_iri.len());
            output_buffer.push_str(&base_iri[..base_p.authority_end]);
            write_path_without_dot_segments_to(&rel_iri[..rel_p.path_end], output_buffer, base_p.authority_end, false);
        } else if base_p.authority_end > base_p.scheme_end && base_p.authority_end == base_p.path_end {
            output_buffer.reserve_exact(base_p.authority_end + 1 + (rel_iri.len() - rel_p.authority_end));
            output_buffer.push_str(&base_iri[..base_p.authority_end]);
            write_path_without_dot_segments_to(&rel_iri[rel_p.authority_end..rel_p.path_end], output_buffer, base_p.authority_end, true);
        } else if let Some(last_slash) = memrchr(b'/', &base_iri.as_bytes()[base_p.authority_end..base_p.path_end]) {
            output_buffer.reserve_exact(base_p.authority_end + last_slash + (rel_iri.len() - rel_p.authority_end) + 1);
            output_buffer.push_str(&base_iri[..base_p.authority_end]);
            if base_p.authority_end > 0 {
                write_path_without_dot_segments_to(&base_iri[base_p.authority_end..][..last_slash + 1], output_buffer, base_p.authority_end, false);
                let with_prefix_slash = if output_buffer.ends_with('/') {
                    output_buffer.pop();
                    true
                } else {
                    false
                };
                write_path_without_dot_segments_to(
                    &rel_iri[rel_p.authority_end..rel_p.path_end],
                    output_buffer,
                    base_p.authority_end,
                    with_prefix_slash,
                );
            } else {
                output_buffer.push_str(&base_iri[base_p.authority_end..][..last_slash + 1]);
                output_buffer.push_str(&rel_iri[rel_p.authority_end..rel_p.path_end]);
            }
        } else {
            output_buffer.reserve_exact(base_p.authority_end + (rel_iri.len() - rel_p.authority_end));
            output_buffer.push_str(&base_iri[..base_p.authority_end]);
            write_path_without_dot_segments_to(&rel_iri[rel_p.authority_end..rel_p.path_end], output_buffer, base_p.authority_end, false);
        }
        let path_end = output_buffer.len();
        output_buffer.push_str(&rel_iri[rel_p.path_end..]);
        return Positions {
            scheme_end: base_p.scheme_end,
            authority_end: base_p.authority_end,
            path_end,
            query_end: path_end + (rel_p.query_end - rel_p.path_end),
        };
    }
    if rel_p.query_end > 0 {
        output_buffer.reserve_exact(base_p.path_end + rel_iri.len());
        output_buffer.push_str(&base_iri[..base_p.path_end]);
        output_buffer.push_str(rel_iri);
        return Positions {
            scheme_end: base_p.scheme_end,
            authority_end: base_p.authority_end,
            path_end: base_p.path_end,
            query_end: base_p.path_end + rel_p.query_end,
        };
    }
    output_buffer.reserve_exact(base_p.query_end + rel_iri.len());
    output_buffer.push_str(&base_iri[..base_p.query_end]);
    output_buffer.push_str(rel_iri);
    base_p
}

/// Checks whether the segment of `path` starting at `start` is a complete `.`
/// or `..` segment.
///
/// `start` must be the index of the first byte of a segment, i.e. either `0` or
/// one past a `/`.
#[inline]
const fn segment_is_dots(path: &[u8], start: usize) -> bool {
    // `const fn` forbids `matches!` over `Option<&u8>` patterns bound by a
    // slice index, so the bounds are checked explicitly.
    if start >= path.len() || path[start] != b'.' {
        return false;
    }
    if start + 1 == path.len() || path[start + 1] == b'/' {
        return true;
    }
    if path[start + 1] != b'.' {
        return false;
    }
    start + 2 == path.len() || path[start + 2] == b'/'
}

/// Checks whether `path` contains a `.` or `..` segment, i.e. whether RFC 3986
/// §5.2.4 `remove_dot_segments` would change it.
///
/// This guards the hot path of [`resolve`]: most references have a path with no
/// `.` byte at all, and a single SIMD scan rejects them outright so the caller
/// can keep its verbatim copy instead of walking the path segment by segment.
fn path_has_dot_segments(path: &str) -> bool {
    let path = path.as_bytes();
    if memchr(b'.', path).is_none() {
        return false;
    }
    if segment_is_dots(path, 0) {
        return true;
    }
    let mut offset = 0;
    while let Some(index) = memmem::find(&path[offset..], b"/.") {
        // `+ 1` skips the `/`, landing on the first byte of the segment. The
        // offset therefore advances by at least one byte per iteration.
        let segment_start = offset + index + 1;
        if segment_is_dots(path, segment_start) {
            return true;
        }
        offset = segment_start;
    }
    false
}

fn write_path_without_dot_segments_to(mut input: &str, output: &mut String, output_path_start: usize, with_prefix_slash: bool) {
    if with_prefix_slash {
        if input.starts_with("./") {
            input = &input[1..];
        } else if input == "." {
            input = "/";
        } else if input.starts_with("../") {
            input = &input[2..];
            remove_last_segment(output, output_path_start);
        } else if input == ".." {
            input = "/";
            remove_last_segment(output, output_path_start);
        } else {
            output.push('/');
            let slash = memchr(b'/', input.as_bytes()).unwrap_or(input.len());
            output.push_str(&input[..slash]);
            input = &input[slash..];
        }
    }
    while !input.is_empty() {
        if let Some(rest) = input.strip_prefix("../") {
            input = rest;
        } else if let Some(rest) = input.strip_prefix("./") {
            input = rest;
        } else if input.starts_with("/./") {
            input = &input[2..];
        } else if input == "/." {
            input = "/";
        } else if input.starts_with("/../") {
            input = &input[3..];
            remove_last_segment(output, output_path_start);
        } else if input == "/.." {
            input = "/";
            remove_last_segment(output, output_path_start);
        } else if input == "." || input == ".." {
            input = "";
        } else {
            input = if let Some(rest) = input.strip_prefix('/') {
                output.push('/');
                rest
            } else {
                input
            };
            let slash = memchr(b'/', input.as_bytes()).unwrap_or(input.len());
            output.push_str(&input[..slash]);
            input = &input[slash..];
        }
    }
}

fn remove_last_segment(output: &mut String, output_path_start: usize) {
    let last = memrchr(b'/', &output.as_bytes()[output_path_start..]).unwrap_or(0);
    output.truncate(output_path_start + last);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{find_iri_positions, find_iri_ref_positions};

    /// RFC 3986 §5.4 base against which the reference vectors are resolved.
    const RFC3986_BASE: &str = "http://a/b/c/d;p?q";

    /// `@base` of the W3C JSON-LD expansion test `#t0062`.
    const T0062_BASE: &str = "http://example.com/some/deep/directory/and/file#with-a-fragment";

    /// Resolves `reference` against `base` and asserts that the [`Positions`]
    /// returned by [`resolve`] describe the produced string, which is the part
    /// of the contract a caller cannot re-derive cheaply.
    fn resolved(base: &str, reference: &str) -> String {
        let base_p = find_iri_positions(base);
        let rel_p = find_iri_ref_positions(reference);
        let mut output = String::new();
        let positions = resolve((base, base_p), (reference, rel_p), &mut output);
        assert_eq!(
            positions,
            find_iri_positions(&output),
            "positions returned for `{reference}` against `{base}` do not describe `{output}`"
        );
        output
    }

    #[test]
    fn rfc3986_normal_references_resolve() {
        for (reference, expected) in [
            ("g:h", "g:h"),
            ("g", "http://a/b/c/g"),
            ("./g", "http://a/b/c/g"),
            ("g/", "http://a/b/c/g/"),
            ("/g", "http://a/g"),
            ("//g", "http://g"),
            ("?y", "http://a/b/c/d;p?y"),
            ("g?y", "http://a/b/c/g?y"),
            ("#s", "http://a/b/c/d;p?q#s"),
            ("g#s", "http://a/b/c/g#s"),
            ("g?y#s", "http://a/b/c/g?y#s"),
            (";x", "http://a/b/c/;x"),
            ("g;x", "http://a/b/c/g;x"),
            ("g;x?y#s", "http://a/b/c/g;x?y#s"),
            ("", "http://a/b/c/d;p?q"),
            (".", "http://a/b/c/"),
            ("./", "http://a/b/c/"),
            ("..", "http://a/b/"),
            ("../", "http://a/b/"),
            ("../g", "http://a/b/g"),
            ("../..", "http://a/"),
            ("../../", "http://a/"),
            ("../../g", "http://a/g"),
        ] {
            assert_eq!(resolved(RFC3986_BASE, reference), expected, "reference `{reference}`");
        }
    }

    #[test]
    fn rfc3986_abnormal_references_resolve() {
        for (reference, expected) in [
            ("../../../g", "http://a/g"),
            ("../../../../g", "http://a/g"),
            ("/./g", "http://a/g"),
            ("/../g", "http://a/g"),
            ("g.", "http://a/b/c/g."),
            (".g", "http://a/b/c/.g"),
            ("g..", "http://a/b/c/g.."),
            ("..g", "http://a/b/c/..g"),
            ("./../g", "http://a/b/g"),
            ("./g/.", "http://a/b/c/g/"),
            ("g/./h", "http://a/b/c/g/h"),
            ("g/../h", "http://a/b/c/h"),
            ("g;x=1/./y", "http://a/b/c/g;x=1/y"),
            ("g;x=1/../y", "http://a/b/c/y"),
            ("g?y/./x", "http://a/b/c/g?y/./x"),
            ("g?y/../x", "http://a/b/c/g?y/../x"),
            ("g#s/./x", "http://a/b/c/g#s/./x"),
            ("g#s/../x", "http://a/b/c/g#s/../x"),
        ] {
            assert_eq!(resolved(RFC3986_BASE, reference), expected, "reference `{reference}`");
        }
    }

    #[test]
    fn network_path_reference_removes_dot_segments() {
        // The two entries of the W3C `#t0062` expansion test that a verbatim
        // copy of the reference got wrong.
        assert_eq!(resolved(T0062_BASE, "//example.org/../scheme-relative"), "http://example.org/scheme-relative");
        assert_eq!(
            resolved(T0062_BASE, "//example.org/.././useless/../../scheme-relative"),
            "http://example.org/scheme-relative"
        );
    }

    #[test]
    fn network_path_reference_without_dot_segments_is_copied_verbatim() {
        assert_eq!(resolved(T0062_BASE, "//example.org/scheme-relative"), "http://example.org/scheme-relative");
        assert_eq!(resolved(RFC3986_BASE, "//g/a.b/c..d"), "http://g/a.b/c..d");
    }

    #[test]
    fn network_path_reference_keeps_query_and_fragment_after_dot_removal() {
        assert_eq!(resolved(RFC3986_BASE, "//g/a/../b?y=1#s"), "http://g/b?y=1#s");
    }

    #[test]
    fn absolute_reference_removes_dot_segments() {
        assert_eq!(resolved(RFC3986_BASE, "http://a/b/../c"), "http://a/c");
        assert_eq!(resolved(RFC3986_BASE, "http://a/./b/./c"), "http://a/b/c");
        assert_eq!(resolved(RFC3986_BASE, "http://a/b/c/../../.."), "http://a/");
    }

    #[test]
    fn absolute_reference_without_dot_segments_is_copied_verbatim() {
        assert_eq!(resolved(RFC3986_BASE, "https://example.com/a/b?q=.#f."), "https://example.com/a/b?q=.#f.");
        assert_eq!(resolved(RFC3986_BASE, "http://a/b.c/d..e/.f"), "http://a/b.c/d..e/.f");
    }

    #[test]
    fn absolute_reference_keeps_query_and_fragment_after_dot_removal() {
        assert_eq!(resolved(RFC3986_BASE, "http://a/b/../c?y=1#s"), "http://a/c?y=1#s");
    }

    #[test]
    fn absolute_rootless_reference_removes_dot_segments() {
        // RFC 3986 §5.2.4 applied to a rootless path: `foo` is dropped by the
        // following `..`, leaving an absolute path.
        assert_eq!(resolved(RFC3986_BASE, "urn:foo/../bar"), "urn:/bar");
    }

    #[test]
    fn dot_segment_detection_rejects_paths_without_dot_segments() {
        for path in [
            "", "/", "/a/b/c", "/a.b/c.d", "/..a/b", "/a../b", "/a/..b", "/a/b..", "/...", "/a/.../b", "foo", ".foo", "..foo",
        ] {
            assert!(!path_has_dot_segments(path), "path `{path}`");
        }
    }

    #[test]
    fn dot_segment_detection_accepts_paths_with_dot_segments() {
        for path in [
            ".",
            "..",
            "./",
            "../",
            "./a",
            "../a",
            "/.",
            "/..",
            "/./",
            "/../",
            "/a/./b",
            "/a/../b",
            "/a/.",
            "/a/..",
            "/a.b/../c",
        ] {
            assert!(path_has_dot_segments(path), "path `{path}`");
        }
    }
}
