//! Helper views over path / authority components, and convenience accessors.

use std::ops::Deref;

use crate::types::{Iri, IriRef, Uri, UriRef};

#[inline]
#[must_use]
pub fn path_segments(path: &str) -> PathSegments<'_> {
    let rest = path.strip_prefix('/').unwrap_or(path);
    PathSegments {
        inner: if rest.is_empty() { None } else { Some(rest.split('/')) },
    }
}

pub struct PathSegments<'a> {
    inner: Option<std::str::Split<'a, char>>,
}

impl<'a> Iterator for PathSegments<'a> {
    type Item = &'a str;
    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        self.inner.as_mut()?.next()
    }
}

impl<'a> DoubleEndedIterator for PathSegments<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a str> {
        self.inner.as_mut()?.next_back()
    }
}

#[inline]
#[must_use]
pub fn path_is_absolute(path: &str) -> bool {
    path.starts_with('/')
}

/// Applies RFC 3986 §5.2.4 `remove_dot_segments` to a standalone path.
///
/// This differs from the resolution-time pass in [`crate::resolve`] in how it
/// treats a `..` with nothing left to climb over. Resolution always runs
/// against a base, so such a `..` is spent and discarded. A standalone relative
/// path has no base yet, and the `..` still applies to whatever it is later
/// resolved against — dropping it would make `../a` and `a` normalize alike.
/// It is therefore preserved (`a/../..` → `../`), while on an absolute path the
/// root absorbs it (`/a/../..` → `/`) as usual.
///
/// A leading empty segment is likewise kept: `//a` and `/a` name different
/// resources under an authority, so collapsing them would make distinct IRIs
/// compare equal.
///
/// Nothing here guards a first segment holding a `:`, unlike upstream `iref`'s
/// context-free `Path::normalized`. This runs on the path of a *parsed* IRI,
/// whose scheme has already settled the ambiguity — guarding `urn:isbn:0451`'s
/// `isbn:0451` would rewrite a path that was never in doubt, and force every
/// `urn:`/`tag:`/`mailto:` comparison through an allocation. The two places
/// that do drop the context, recomposition in [`crate::mutate`] (and in
/// [`crate::normalize`]) and [`crate::relativize`], add the `./` themselves;
/// recomposition does it through the crate-internal `path_guard`.
#[must_use]
pub fn normalize_path(path: &str) -> String {
    if path_is_normalized(path) {
        return path.to_owned();
    }
    remove_dot_segments(path)
}

/// The dot-segment removal loop of [`normalize_path`], without its shortcut.
fn remove_dot_segments(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut input = path;
    let mut out = String::with_capacity(path.len());
    // Bytes of `out` holding `..` segments this path could not resolve away. A
    // later `..` must not pop them, or `../..` would collapse back to `../`.
    let mut floor = 0usize;

    // Cheap prefix tests come first and `memchr` runs only for a real segment:
    // dot segments are short, and a SIMD scan costs more to set up than the
    // comparisons that recognise them.
    while !input.is_empty() {
        if input.starts_with("../") {
            // Leading `..` on a relative path — nothing precedes it to remove,
            // so `pop_segment` preserves it. Consume only the dots and leave
            // the `/` to separate it from whatever follows.
            input = &input[2..];
            pop_segment(&mut out, &mut floor, absolute);
        } else if let Some(rest) = input.strip_prefix("./") {
            input = rest;
        } else if input.starts_with("/./") {
            input = &input[2..];
        } else if input == "/." {
            input = "/";
        } else if input.starts_with("/../") {
            input = &input[3..];
            pop_segment(&mut out, &mut floor, absolute);
        } else if input == "/.." || input == ".." {
            pop_segment(&mut out, &mut floor, absolute);
            input = close_directory(&out, absolute);
        } else if input == "." {
            input = "";
        } else {
            let rest = if let Some(r) = input.strip_prefix('/') {
                // A relative path just emptied by `..` must not pick up a
                // leading `/` and turn absolute.
                if absolute || !out.is_empty() {
                    out.push('/');
                }
                r
            } else {
                input
            };
            let end = memchr::memchr(b'/', rest.as_bytes()).unwrap_or(rest.len());
            out.push_str(&rest[..end]);
            input = &rest[end..];
        }
    }
    out
}

/// A path ending in a dot segment names a directory and so keeps a trailing
/// `/` — unless that would make an emptied relative path absolute.
#[inline]
fn close_directory(out: &str, absolute: bool) -> &'static str {
    if out.is_empty() && !absolute { "" } else { "/" }
}

/// Whether [`normalize_path`] would return `path` unchanged, judged by
/// whether any segment is `.` or `..`.
///
/// The single source of truth for that question: callers on the comparison and
/// hashing paths use it to skip the allocation. It may be conservative (`../a`
/// has a `..` that normalization keeps) but never optimistic, or equal IRIs
/// would hash apart. Dots inside a segment (`n.html`, `.well-known`, `a..b`)
/// do not count.
#[inline]
pub(crate) fn path_is_normalized(path: &str) -> bool {
    let bytes = path.as_bytes();
    memchr::memchr_iter(b'.', bytes).all(|i| {
        // A dot segment starts at a segment start, holds one or two dots,
        // and ends at a `/` or the end of the path.
        if i > 0 && bytes[i - 1] != b'/' {
            return true;
        }
        let rest = &bytes[i + 1..];
        let rest = rest.strip_prefix(b".").unwrap_or(rest);
        !(rest.is_empty() || rest[0] == b'/')
    })
}

/// Whether `path` is a relative path whose first segment holds a `:`, which
/// RFC 3986 §4.2 would read as a scheme unless a `./` precedes it.
///
/// An absolute path — every path under an authority — settles on the first
/// byte, so this costs a single comparison on the common path.
#[inline]
pub(crate) fn first_segment_has_colon(path: &str) -> bool {
    if path.starts_with('/') {
        return false;
    }
    let first_segment = match memchr::memchr(b'/', path.as_bytes()) {
        Some(slash) => &path[..slash],
        None => path,
    };
    memchr::memchr(b':', first_segment.as_bytes()).is_some()
}

/// The dot segment that has to precede `path` when it is written after the
/// given scheme and authority, so that it reads back as the same path.
///
/// Both cases are recoverable with a dot segment, which denotes the same path
/// but cannot be misread on the way back in.
#[inline]
pub(crate) fn path_guard(has_scheme: bool, has_authority: bool, path: &str) -> &'static str {
    if has_authority {
        ""
    } else if path.starts_with("//") {
        // RFC 3986 §3.3: with no authority, a path may not begin with `//`.
        "/."
    } else if !has_scheme && first_segment_has_colon(path) {
        // RFC 3986 §4.2: the first segment of a relative-path reference may not
        // hold a `:`, or it reads as a scheme.
        "./"
    } else {
        ""
    }
}

fn pop_segment(out: &mut String, floor: &mut usize, absolute: bool) {
    if out.len() > *floor {
        match memchr::memrchr(b'/', &out.as_bytes()[*floor..]) {
            Some(slash) => out.truncate(*floor + slash),
            None => out.truncate(*floor),
        }
        return;
    }
    // Nothing left to climb over. On a relative path the `..` still applies to
    // whatever base the path is resolved against, so it has to survive; an
    // absolute path's root absorbs it.
    if !absolute {
        if *floor > 0 {
            out.push('/');
        }
        out.push_str("..");
        *floor = out.len();
    }
}

#[must_use]
pub fn split_authority(authority: &str) -> (Option<&str>, &str, Option<&str>) {
    let (user_info, rest) = match memchr::memchr(b'@', authority.as_bytes()) {
        Some(i) => (Some(&authority[..i]), &authority[i + 1..]),
        None => (None, authority),
    };
    let (host, port) = if let Some(rest_in) = rest.strip_prefix('[') {
        if let Some(end) = memchr::memchr(b']', rest_in.as_bytes()) {
            let host = &rest[..end + 2];
            let tail = &rest[end + 2..];
            match tail.strip_prefix(':') {
                Some(p) => (host, Some(p)),
                None => (host, None),
            }
        } else {
            (rest, None)
        }
    } else {
        match memchr::memchr(b':', rest.as_bytes()) {
            Some(i) => (&rest[..i], Some(&rest[i + 1..])),
            None => (rest, None),
        }
    };
    (user_info, host, port)
}

impl<T: Deref<Target = str>> Iri<T> {
    pub fn path_segments(&self) -> PathSegments<'_> {
        path_segments(self.path())
    }

    pub fn authority_parts(&self) -> Option<(Option<&str>, &str, Option<&str>)> {
        self.authority().map(split_authority)
    }
}

impl<T: Deref<Target = str>> IriRef<T> {
    pub fn path_segments(&self) -> PathSegments<'_> {
        path_segments(self.path())
    }
    pub fn authority_parts(&self) -> Option<(Option<&str>, &str, Option<&str>)> {
        self.authority().map(split_authority)
    }
}

impl<T: Deref<Target = str>> Uri<T> {
    pub fn path_segments(&self) -> PathSegments<'_> {
        path_segments(self.path())
    }
    pub fn authority_parts(&self) -> Option<(Option<&str>, &str, Option<&str>)> {
        self.authority().map(split_authority)
    }
}

impl<T: Deref<Target = str>> UriRef<T> {
    pub fn path_segments(&self) -> PathSegments<'_> {
        path_segments(self.path())
    }
    pub fn authority_parts(&self) -> Option<(Option<&str>, &str, Option<&str>)> {
        self.authority().map(split_authority)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_absolute() {
        let segs: Vec<&str> = path_segments("/a/b/c").collect();
        assert_eq!(segs, vec!["a", "b", "c"]);
    }

    #[test]
    fn segments_relative() {
        let segs: Vec<&str> = path_segments("a/b/c").collect();
        assert_eq!(segs, vec!["a", "b", "c"]);
    }

    #[test]
    fn segments_trailing_slash() {
        let segs: Vec<&str> = path_segments("/a/b/").collect();
        assert_eq!(segs, vec!["a", "b", ""]);
    }

    #[test]
    fn split_auth_host_only() {
        assert_eq!(split_authority("example.com"), (None, "example.com", None));
    }

    #[test]
    fn split_auth_full() {
        assert_eq!(split_authority("user:pass@host:80"), (Some("user:pass"), "host", Some("80")));
    }

    #[test]
    fn split_auth_ipv6() {
        assert_eq!(split_authority("[::1]:8080"), (None, "[::1]", Some("8080")));
    }

    #[test]
    fn normalize_basic() {
        assert_eq!(normalize_path("/a/b/../c"), "/a/c");
        assert_eq!(normalize_path("a/./b/../c"), "a/c");
    }

    #[test]
    fn normalize_absolute_paths() {
        for (input, expected) in [
            ("/", "/"),
            ("/.", "/"),
            ("/./", "/"),
            ("/..", "/"),
            ("/a/..", "/"),
            ("/a/./", "/a/"),
            ("/a/../..", "/"),
            ("/../..", "/"),
            ("/a/b/c/../../d", "/a/d"),
        ] {
            assert_eq!(normalize_path(input), expected, "path `{input}`");
        }
    }

    /// A `..` with nothing left to climb over survives on a relative path: it
    /// still applies to whatever base the path is later resolved against.
    #[test]
    fn normalize_keeps_unresolvable_parent_segments_on_relative_paths() {
        for (input, expected) in [
            ("..", "../"),
            ("../", "../"),
            ("../a", "../a"),
            ("../..", "../../"),
            ("../../", "../../"),
            ("a/../..", "../"),
            ("../a/..", "../"),
            ("a/../../b", "../b"),
        ] {
            assert_eq!(normalize_path(input), expected, "path `{input}`");
        }
    }

    #[test]
    fn normalize_relative_paths() {
        for (input, expected) in [
            ("", ""),
            (".", ""),
            ("./", ""),
            ("./a", "a"),
            ("a/..", ""),
            ("a/.", "a/"),
            ("a/b/..", "a/"),
            ("a/b/../", "a/"),
            ("a/b/c/..", "a/b/"),
            ("a/b/c/.", "a/b/c/"),
            ("a/../b", "b"),
            ("x/./y/.././z", "x/z"),
        ] {
            assert_eq!(normalize_path(input), expected, "path `{input}`");
        }
    }

    /// Empty segments name real path components: `//a` and `/a` are different
    /// resources under an authority, so normalization must not merge them.
    #[test]
    fn normalize_preserves_empty_segments() {
        for (input, expected) in [
            ("a//b", "a//b"),
            ("a//../b", "a/b"),
            ("//a", "//a"),
            ("/.//a", "//a"),
            ("/.//", "//"),
            ("/a/..//b", "//b"),
        ] {
            assert_eq!(normalize_path(input), expected, "path `{input}`");
        }
    }

    /// A `:` in the first segment is left alone: this normalizes the path of an
    /// already-parsed IRI, where the scheme has settled what the segment is.
    /// Guarding it here would rewrite unambiguous `urn:`/`tag:` paths and cost
    /// an allocation on every such comparison; the callers that genuinely lose
    /// the context add the `./` themselves.
    #[test]
    fn normalize_leaves_a_first_segment_colon_alone() {
        for (input, expected) in [
            ("a:b", "a:b"),
            ("./a:b", "a:b"),
            ("x/../a:b", "a:b"),
            ("a/b:c", "a/b:c"),
            ("./a/b:c", "a/b:c"),
            ("/a:b", "/a:b"),
            // The corpus paths this keeps off the allocating path.
            ("isbn:0451450523", "isbn:0451450523"),
            ("example.com,2026-01-01:foo/bar", "example.com,2026-01-01:foo/bar"),
        ] {
            assert_eq!(normalize_path(input), expected, "path `{input}`");
        }
    }

    /// The guard still exists for callers that do need it.
    #[test]
    fn first_segment_colon_is_detectable() {
        for (path, expected) in [("a:b", true), ("a/b:c", false), ("/a:b", false), ("", false), ("a", false)] {
            assert_eq!(first_segment_has_colon(path), expected, "path `{path}`");
        }
    }

    /// `path_is_normalized` never claims a path normal that the removal loop
    /// would change, and does not mistake dots inside a segment for dot
    /// segments.
    #[test]
    fn path_is_normalized_is_never_optimistic() {
        const POOL: &[char] = &['.', '.', '.', '/', '/', 'a', ':'];
        for path in ["n.html", "/a/.well-known/x", "a..b", "/...", "/.a/", "a./b", "/a/b.c/d"] {
            assert!(path_is_normalized(path), "`{path}`");
        }
        for path in ["/a/.", "/a/..", "./a", ".", "..", "/./", "a/./b", "/a/../b"] {
            assert!(!path_is_normalized(path), "`{path}`");
        }
        let mut state: u32 = 0x5eed_0001;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as usize
        };
        for _ in 0..50_000 {
            let path: String = (0..next() % 12).map(|_| POOL[next() % POOL.len()]).collect();
            if path_is_normalized(&path) {
                assert_eq!(remove_dot_segments(&path), path, "`{path}` was claimed normal");
            }
        }
    }

    #[test]
    fn normalize_is_idempotent() {
        for input in [
            "",
            "/",
            ".",
            "..",
            "./",
            "../",
            "./a",
            "../a",
            "a/..",
            "a/../..",
            "/a/../..",
            "/..",
            "/./",
            "a/b/..",
            "a//b",
            "/.//",
            "a:b",
            "./a:b",
            "x/./y/.././z",
        ] {
            let once = normalize_path(input);
            assert_eq!(normalize_path(&once), once, "path `{input}`");
        }
    }
}
