//! RFC 3986/3987 §6.2.2 syntax-based normalization: the normal form, and the
//! streaming comparison that agrees with it.
//!
//! This is the RFC 3987 §5.3.2 rung of the comparison ladder. `==`, `Hash`
//! and `Ord` on the IRI types are the §5.3.1 rung, Simple String Comparison,
//! and never come through here. `equivalent()` and `normalized()` do.
//!
//! The normal form is defined once, by `write_normalized`, which `normalized()`
//! runs. [`iri_eq`] compares the same canonical components without building the
//! whole form, and the tests hold it to `normalized()`.
//!
//! Per component:
//! * scheme: ASCII-lowercased;
//! * user info: percent-normalized, case kept;
//! * host: percent-normalized, then ASCII-lowercased outside the remaining
//!   `%XX` triplets;
//! * port: kept as written;
//! * path: percent-normalized, then dot segments removed. Under a scheme or
//!   an authority that is RFC 3986 §5.2.4 `remove_dot_segments` as resolution
//!   runs it, so `foo:a/../../b` is `foo:/b`, what it resolves to. A
//!   relative-path reference (no scheme, no authority) goes through
//!   [`normalize_path`] instead, which keeps a `..` with nothing to climb
//!   over, since it still climbs the base it is resolved against. One whose
//!   path that empties becomes `./`: against a base, `.` and `a/..` name the
//!   base's directory, while an empty path names the base itself;
//! * query, fragment: percent-normalized, case and order kept.
//!
//! Percent-normalization decodes escaped unreserved characters (letters,
//! digits, `-` `.` `_` `~`) and uppercases the hex digits of every other
//! escape. It runs before dot-segment removal, so `%2E%2E` is a `..` segment.

use std::{borrow::Cow, ops::Deref};

use pct::rfc3986::{Chunk, chunks};

use crate::{
    components::{normalize_path, path_guard, path_is_normalized, split_authority},
    parse::Positions,
    resolve::remove_dot_segments,
    types::{Iri, IriRef, Uri, UriRef},
};

/// The components of an IRI reference, as the comparison sees them.
struct Parts<'a> {
    scheme: Option<&'a str>,
    /// `Some("")` for an empty authority (`file:///`), `None` for none.
    authority: Option<&'a str>,
    path: &'a str,
    query: Option<&'a str>,
    fragment: Option<&'a str>,
}

impl<'a> Parts<'a> {
    fn of(s: &'a str, p: Positions) -> Self {
        Self {
            scheme: (p.scheme_end > 0).then(|| &s[..p.scheme_end - 1]),
            authority: (p.authority_end > p.scheme_end).then(|| &s[p.scheme_end + 2..p.authority_end]),
            path: &s[p.authority_end..p.path_end],
            query: (p.query_end > p.path_end).then(|| &s[p.path_end + 1..p.query_end]),
            fragment: (s.len() > p.query_end).then(|| &s[p.query_end + 1..]),
        }
    }
}

/// Whether two IRI references have the same RFC 3986/3987 §6.2.2 normal form,
/// compared component by component without building either form.
#[must_use]
pub fn iri_eq(a: &str, pa: Positions, b: &str, pb: Positions) -> bool {
    if a == b {
        return true;
    }
    let (x, y) = (Parts::of(a, pa), Parts::of(b, pb));
    // The path is only reached once the schemes and authorities matched, so
    // `x` alone says whether both belong to relative-path references.
    let relative = x.scheme.is_none() && x.authority.is_none();
    opt_eq(x.scheme, y.scheme, str::eq_ignore_ascii_case)
        && opt_eq(x.authority, y.authority, authority_eq)
        && path_eq(x.path, y.path, relative)
        && opt_eq(x.query, y.query, pct_eq)
        && opt_eq(x.fragment, y.fragment, pct_eq)
}

fn opt_eq(a: Option<&str>, b: Option<&str>, eq: impl FnOnce(&str, &str) -> bool) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => eq(x, y),
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn authority_eq(a: &str, b: &str) -> bool {
    let (ui_a, host_a, port_a) = split_authority(a);
    let (ui_b, host_b, port_b) = split_authority(b);
    port_a == port_b && opt_eq(ui_a, ui_b, pct_eq) && host_eq(host_a, host_b)
}

fn host_eq(a: &str, b: &str) -> bool {
    if memchr::memchr(b'%', a.as_bytes()).is_none() && memchr::memchr(b'%', b.as_bytes()).is_none() {
        return a.eq_ignore_ascii_case(b);
    }
    canonical(a, true) == canonical(b, true)
}

fn path_eq(a: &str, b: &str, relative: bool) -> bool {
    a == b || canonical_path(a, relative) == canonical_path(b, relative)
}

fn pct_eq(a: &str, b: &str) -> bool {
    a == b || canonical(a, false) == canonical(b, false)
}

/// Appends `s` with its ASCII letters lowercased.
fn push_lower(out: &mut String, s: &str) {
    let start = out.len();
    out.push_str(s);
    out[start..].make_ascii_lowercase();
}

/// Writes the normal form of the IRI reference `iri` (see the module docs).
///
/// With `scheme_defaults`, the RFC 3986 §6.2.3 scheme-based steps are applied
/// too: an empty port is dropped for every scheme, and for `http` and `https`
/// the default port is dropped and an empty path under an authority becomes
/// `/`. [`iri_eq`] never uses them.
fn write_normalized(iri: &str, p: Positions, scheme_defaults: bool, out: &mut String) {
    let scheme = (p.scheme_end > 0).then(|| &iri[..p.scheme_end - 1]);
    if let Some(scheme) = scheme {
        push_lower(out, scheme);
        out.push(':');
    }
    let default_port = match scheme {
        Some(s) if scheme_defaults && s.eq_ignore_ascii_case("http") => Some("80"),
        Some(s) if scheme_defaults && s.eq_ignore_ascii_case("https") => Some("443"),
        _ => None,
    };
    let has_authority = p.authority_end > p.scheme_end;
    if has_authority {
        out.push_str("//");
        let (user_info, host, port) = split_authority(&iri[p.scheme_end + 2..p.authority_end]);
        if let Some(user_info) = user_info {
            push_pct_normalized(out, user_info, false);
            out.push('@');
        }
        push_pct_normalized(out, host, true);
        if let Some(port) = port.filter(|&port| !(scheme_defaults && port.is_empty()) && Some(port) != default_port) {
            out.push(':');
            out.push_str(port);
        }
    }
    let path = canonical_path(&iri[p.authority_end..p.path_end], scheme.is_none() && !has_authority);
    if path.is_empty() && has_authority && default_port.is_some() {
        out.push('/');
    } else {
        out.push_str(path_guard(scheme.is_some(), has_authority, &path));
        out.push_str(&path);
    }
    if p.query_end > p.path_end {
        out.push('?');
        push_pct_normalized(out, &iri[p.path_end + 1..p.query_end], false);
    }
    if iri.len() > p.query_end {
        out.push('#');
        push_pct_normalized(out, &iri[p.query_end + 1..], false);
    }
}

/// Appends `s` percent-normalized. With `lower`, ASCII letters are lowercased
/// too, except the hex digits of the escapes that stay encoded.
///
/// How an escape normalizes is [`pct::rfc3986`]'s decision. It needs no
/// UTF-8 from the decoded octets (`%FF` is a valid escape), and encodes a `%`
/// that starts no escape, which only an unchecked parse can produce, as `%25`.
/// The comparison and `normalized()` both go through here.
fn push_pct_normalized(out: &mut String, s: &str, lower: bool) {
    for chunk in chunks(s) {
        match chunk {
            Chunk::Literal(run) if lower => push_lower(out, run),
            Chunk::Literal(run) => out.push_str(run),
            Chunk::Decoded(byte) => out.push(char::from(if lower { byte.to_ascii_lowercase() } else { byte })),
            Chunk::Encoded(escape) => out.extend(escape.into_iter().map(char::from)),
        }
    }
}

/// `s` percent-normalized, borrowed when that changes nothing.
fn canonical(s: &str, lower: bool) -> Cow<'_, str> {
    if memchr::memchr(b'%', s.as_bytes()).is_none() && !(lower && s.bytes().any(|b| b.is_ascii_uppercase())) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    push_pct_normalized(&mut out, s, lower);
    Cow::Owned(out)
}

/// The path percent-normalized and then stripped of dot segments.
///
/// Under a scheme or an authority the dot segments go as resolution removes
/// them (RFC 3986 §5.2.4), so the normal form is what the reference resolves
/// to: `foo:a/..` is `foo:`, and `foo:a/../../b` is `foo:/b`.
///
/// With `relative`, the path belongs to a relative-path reference, which has
/// yet to be merged with a base. [`normalize_path`] keeps the `..` segments
/// that would climb that base, and a path that empties keeps a `./`: `.` and
/// `a/..` name the base's directory (`http://a/b/` against `http://a/b/c`),
/// while an empty path names the base itself.
fn canonical_path(path: &str, relative: bool) -> Cow<'_, str> {
    let decoded = canonical(path, false);
    if path_is_normalized(&decoded) {
        return decoded;
    }
    if !relative {
        return Cow::Owned(remove_dot_segments(&decoded));
    }
    let normalized = normalize_path(&decoded);
    if normalized.is_empty() { Cow::Borrowed("./") } else { Cow::Owned(normalized) }
}

fn normalized_string(iri: &str, p: Positions, scheme_defaults: bool) -> String {
    let mut out = String::with_capacity(iri.len());
    write_normalized(iri, p, scheme_defaults, &mut out);
    out
}

/// Generates `equivalent`, `normalized` and `normalized_for_scheme` for one of
/// the four reference types. The normal form is ASCII whenever the input is,
/// so the URI types stay URIs.
macro_rules! normalized_methods {
    ($ty:ident, $field:ident $(. $inner:tt)?) => {
        impl<T: Deref<Target = str>> $ty<T> {
            /// Whether `self` and `other` are equivalent under RFC 3987
            /// §5.3.2 (RFC 3986 §6.2.2) syntax-based normalization: whether
            /// they have the same [`normalized`](Self::normalized) form.
            ///
            /// This is not `==`, which is RFC 3987 §5.3.1 Simple String
            /// Comparison: `HTTP://a/%7e` and `http://a/~` are equivalent but
            /// not equal. Neither form is built; the components are compared
            /// as they are read, and nothing is allocated unless a component
            /// holds an escape, uppercase host letters or dot segments.
            ///
            /// Only escaped unreserved ASCII decodes, so `http://a/%C3%A9` is
            /// not equivalent to the IRI `http://a/é`. RFC 3987 §5.3.2.3
            /// aligns the two by comparing the URIs the IRIs map to, which
            /// `Iri::to_uri` and `IriRef::to_uri_ref` return.
            #[must_use]
            pub fn equivalent<U: Deref<Target = str>>(&self, other: &$ty<U>) -> bool {
                iri_eq(
                    &self$(.$inner)?.$field,
                    self$(.$inner)?.positions,
                    &other$(.$inner)?.$field,
                    other$(.$inner)?.positions,
                )
            }

            /// Returns the RFC 3986/3987 §6.2.2 syntax-based normal form: the
            /// scheme and host lowercased, escaped unreserved characters
            /// decoded, other escapes in uppercase hex, and `.` and `..`
            /// segments removed. The case of the user info, path, query and
            /// fragment is kept, query parameters keep their order, and no
            /// trailing slash is added or removed.
            ///
            /// The result is [`equivalent`](Self::equivalent) to `self`, and
            /// two values are equivalent exactly when their normal forms are
            /// `==`. To key a map or set by normalized identity, normalize
            /// each IRI once and use the result as the key: `==` and `Hash`
            /// on it then compare bytes.
            #[must_use]
            pub fn normalized(&self) -> $ty<String> {
                let s = normalized_string(&self$(.$inner)?.$field, self$(.$inner)?.positions, false);
                $ty::parse_unchecked(s)
            }

            /// Returns [`normalized`](Self::normalized) with the RFC 3986
            /// §6.2.3 scheme-based steps applied on top: an empty port (`a:`)
            /// is dropped, and for `http` and `https` so is the default port
            /// (`:80`, `:443`), and an empty path under an authority becomes
            /// `/`. Other schemes keep everything else as `normalized` leaves
            /// it.
            ///
            /// [`equivalent`](Self::equivalent) is protocol agnostic, so
            /// unlike `normalized` the result need not be equivalent to
            /// `self`: `http://a:80` and `http://a/` are not.
            #[must_use]
            pub fn normalized_for_scheme(&self) -> $ty<String> {
                let s = normalized_string(&self$(.$inner)?.$field, self$(.$inner)?.positions, true);
                $ty::parse_unchecked(s)
            }
        }
    };
}

normalized_methods!(IriRef, iri);
normalized_methods!(Iri, iri.0);
normalized_methods!(UriRef, uri);
normalized_methods!(Uri, uri.0);

#[cfg(test)]
mod tests {
    use std::{
        cmp::Ordering,
        collections::{BTreeSet, HashSet},
        hash::{BuildHasher, Hash, RandomState},
    };

    use super::*;
    use crate::{
        parse::{find_iri_positions, find_iri_ref_positions},
        types::{IriBuf, IriRefBuf, UriBuf, UriRefBuf},
    };

    fn eq(a: &str, b: &str) -> bool {
        let pa = find_iri_ref_positions(a);
        let pb = find_iri_ref_positions(b);
        iri_eq(a, pa, b, pb)
    }

    fn normalized(s: &str) -> String {
        IriRefBuf::new(s).unwrap().normalized().into_inner()
    }

    fn normalized_for_scheme(s: &str) -> String {
        IriRefBuf::new(s).unwrap().normalized_for_scheme().into_inner()
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

    /// A `..` a relative reference cannot resolve away still moves the target
    /// it resolves to, so it may not be normalized out of the comparison.
    #[test]
    fn relative_references_climbing_past_their_root_are_distinct() {
        for (a, b) in [("../a", "a"), ("..", ""), ("a/../..", ""), ("../..", ".."), ("../a", "./a")] {
            assert!(!eq(a, b), "`{a}` should not be equivalent to `{b}`");
        }
    }

    #[test]
    fn equivalent_relative_references_are_recognised() {
        for (a, b) in [("../..", "../../"), ("./a", "a"), ("a/../b", "b"), ("../a/..", "../"), ("x/./y", "x/y")] {
            assert!(eq(a, b), "`{a}` should be equivalent to `{b}`");
        }
    }

    /// A relative-path reference whose path dot removal empties still names
    /// the base's directory, not the base, so it keeps a `./`.
    #[test]
    fn relative_references_naming_the_base_directory_keep_dot_slash() {
        for a in [".", "./", "a/..", "%2E", "a/b/../..", "./."] {
            assert_eq!(normalized(a), "./", "`{a}`");
            assert!(!eq(a, ""), "`{a}` should not be equivalent to the empty reference");
            assert!(eq(a, "./"), "`{a}` should be equivalent to `./`");
        }
        assert_eq!(normalized(".?q"), "./?q");
        assert!(!eq(".?q", "?q"));
        // With a scheme the path is not merged with a base: dot segments go as
        // resolution removes them.
        assert_eq!(normalized("foo:a/.."), "foo:/");
        assert!(eq("foo:.", "foo:"));
    }

    /// Under a scheme, dot segments go exactly as resolution removes them
    /// (RFC 3986 §5.2.4, not Errata 4547), so equivalent IRIs resolve alike.
    #[test]
    fn dot_segments_under_a_scheme_go_as_resolution_removes_them() {
        for (a, expected) in [
            ("foo:a/../../b", "foo:/b"),
            ("foo:../b", "foo:b"),
            ("foo:a/..", "foo:/"),
            ("foo:.", "foo:"),
            ("foo:a/../..", "foo:/"),
            ("foo:a/..//b", "foo:/.//b"),
            ("http://a/../g", "http://a/g"),
            ("http://a/b/../../../g", "http://a/g"),
            ("//h/../g", "//h/g"),
        ] {
            assert_eq!(normalized(a), expected, "`{a}`");
        }
        assert!(!eq("foo:a/../../b", "foo:../b"));
        // Without a scheme or authority the `..` still has a base to climb.
        assert_eq!(normalized("a/../../b"), "../b");
    }

    /// Every IRI with a scheme is equivalent to what it resolves to:
    /// resolution applies the same dot-segment removal as the normal form.
    #[test]
    fn an_iri_is_equivalent_to_its_resolution() {
        const SEGMENTS: &[&str] = &["", "a", "b", ".", "..", "%2E", "%2e%2E", "a:b"];
        let base = IriBuf::new("http://x/y".to_owned()).unwrap();
        let mut state: u32 = 0x00d0_7d07;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as usize
        };
        for _ in 0..20_000 {
            let mut s = String::from(["foo:", "http://h", "foo://"][next() % 3]);
            let rooted = !s.ends_with(':') || next() % 2 == 0;
            for i in 0..next() % 6 {
                if i > 0 || rooted {
                    s.push('/');
                }
                s.push_str(SEGMENTS[next() % SEGMENTS.len()]);
            }
            let Ok(mut buf) = IriRefBuf::new(s.clone()) else { continue };
            buf.resolve(&base).unwrap();
            // `%2E` is no dot segment to resolution, so decode first.
            let mut decoded = IriRefBuf::new(normalized(&s)).unwrap();
            decoded.resolve(&base).unwrap();
            if !s.contains('%') {
                assert!(eq(&s, buf.as_str()), "`{s}` should be equivalent to its resolution `{}`", buf.as_str());
            }
            assert_eq!(decoded.as_str(), normalized(&s), "`{s}`");
        }
    }

    /// Normalizing a reference never changes what it resolves to.
    ///
    /// Left out: a rootless base with no `/` (`foo:`). Merging with it leaves
    /// the reference's path as it is, and §5.2.4 gives `a/..` a leading `/`
    /// there but `./` none. Equivalence cannot depend on the base, so `a/..`
    /// and `./` stay equivalent, as they are against every other base.
    #[test]
    fn normalizing_a_reference_keeps_its_resolution() {
        let bases = ["http://a/b/c", "http://a/b/c/", "http://a", "foo:/a/b", "foo:a/b"];
        let refs = [
            "",
            ".",
            "./",
            "..",
            "../",
            "a/..",
            "a/../..",
            "./a",
            "../a",
            "?q",
            ".?q",
            "#f",
            ".#f",
            "%2E",
            "%2E%2E/a",
            "a/./b/../c",
            "//h/.",
            "//h",
            "/.",
            "/a/..",
        ];
        for base in bases {
            let base = IriBuf::new(base.to_owned()).unwrap();
            for r in refs {
                let resolve = |s: &str| {
                    let mut buf = IriRefBuf::new(s.to_owned()).unwrap();
                    buf.resolve(&base).unwrap();
                    buf
                };
                let (plain, norm) = (resolve(r), resolve(&normalized(r)));
                assert!(
                    plain.equivalent(&norm),
                    "`{r}` resolves to `{}`, its normal form `{}` to `{}`",
                    plain.as_str(),
                    normalized(r),
                    norm.as_str()
                );
            }
        }
    }

    /// An empty segment is a path component of its own: under an authority
    /// `//a` and `/a` name different resources.
    #[test]
    fn empty_path_segments_are_significant() {
        assert!(!eq("http://host//a", "http://host/a"));
        assert!(eq("http://host/b/..//a", "http://host//a"));
    }

    /// The comparison skips normalizing a path that `path_is_normalized`
    /// vouches for. Vouching wrongly would compare an unnormalized path and
    /// tell equivalent IRIs apart, so the shortcut may be conservative (`./a:b`
    /// is already normal but is not recognised as such) yet never optimistic.
    #[test]
    fn path_shortcut_is_never_optimistic() {
        for path in [
            "", "/", "/a/b", "a:b", "./a:b", "a/b:c", "/a:b", "a/../b", "..", "/.//", "mailto@x", "a//b", "/a/./b",
        ] {
            if path_is_normalized(path) {
                assert_eq!(normalize_path(path), path, "`{path}` was claimed normal but normalization changes it");
            }
        }
    }

    #[test]
    fn paths_needing_a_colon_guard_are_still_equivalent() {
        assert!(eq("urn:a:b", "urn:./a:b"));
    }

    #[test]
    fn normalized_applies_every_syntax_based_step() {
        for (input, expected) in [
            ("HTTP://Example.COM/", "http://example.com/"),
            ("http://a/%7e%2f", "http://a/~%2F"),
            ("http://a/b/./c/../d", "http://a/b/d"),
            ("http://a/?q=%7e%3d#%7Ef", "http://a/?q=~%3D#~f"),
            ("http://exa%6Dple.org/", "http://example.org/"),
            ("http://ex%41mple.org/", "http://example.org/"),
            ("http://A%c3%a9/", "http://a%C3%A9/"),
            ("http://a/%ff%c3", "http://a/%FF%C3"),
            ("http://[::A]:8080/", "http://[::a]:8080/"),
            ("tag:Example.COM,2026:Foo", "tag:Example.COM,2026:Foo"),
        ] {
            assert_eq!(normalized(input), expected, "`{input}`");
        }
    }

    #[test]
    fn normalized_decodes_before_removing_dot_segments() {
        assert_eq!(normalized("http://a/b/%2E%2E/c"), "http://a/c");
        assert!(eq("http://a/b/%2E%2E/c", "http://a/c"));
        assert_eq!(normalized("http://a/b/%2e/c"), "http://a/b/c");
    }

    #[test]
    fn normalized_keeps_what_is_not_normalized() {
        for input in [
            "http://UsEr:PaSs@a/",
            "http://a/Path/To",
            "http://a/p?B=2&a=1",
            "http://a/p#Frag",
            "http://a/p/",
            "http://a/p",
            "http://a",
            "http://a:80/",
            "http://a:/",
            "../a",
            "",
        ] {
            assert_eq!(normalized(input), input, "`{input}`");
        }
    }

    #[test]
    fn normalized_keeps_user_info_case_but_normalizes_its_escapes() {
        assert_eq!(normalized("http://UsEr%7e%2f@a/"), "http://UsEr~%2F@a/");
        assert!(eq("http://UsEr%7e@a/", "http://UsEr~@a/"));
        assert!(!eq("http://user@a/", "http://USER@a/"));
    }

    /// Removing dot segments can leave a path that would read back as another
    /// component; the guard dot segment keeps it a path.
    #[test]
    fn normalized_guards_a_path_that_would_be_misread() {
        for (input, expected) in [("http:/.//a", "http:/.//a"), ("./a:b", "./a:b"), ("x/../a:b", "./a:b")] {
            let out = normalized(input);
            assert_eq!(out, expected, "`{input}`");
            let reparsed = IriRefBuf::new(out.clone()).unwrap();
            assert_eq!(reparsed.path(), IriRefBuf::new(input).unwrap().normalized().path(), "`{input}`");
            assert_eq!(reparsed.authority(), None, "`{input}`");
            assert_eq!(reparsed.scheme(), IriRefBuf::new(input).unwrap().scheme(), "`{input}`");
        }
    }

    #[test]
    fn normalized_for_scheme_applies_http_defaults() {
        for (input, expected) in [
            ("HTTP://a:80", "http://a/"),
            ("http://a:80/p?q", "http://a/p?q"),
            ("https://a:443/p", "https://a/p"),
            ("http://a", "http://a/"),
            ("https://a?q", "https://a/?q"),
            ("http://a:443/", "http://a:443/"),
            ("https://a:80/", "https://a:80/"),
            ("http://a:8080", "http://a:8080/"),
            ("http://a/p/", "http://a/p/"),
            ("ftp://a:21", "ftp://a:21"),
            ("ftp://a", "ftp://a"),
            ("http:p", "http:p"),
            ("//a:80", "//a:80"),
            // §6.2.3: an empty port goes too, whatever the scheme.
            ("http://a:/", "http://a/"),
            ("http://a:", "http://a/"),
            ("ftp://a:/x", "ftp://a/x"),
            ("http://u:p@a:/", "http://u:p@a/"),
            ("http://[::1]:/", "http://[::1]/"),
            ("//a:/", "//a/"),
        ] {
            assert_eq!(normalized_for_scheme(input), expected, "`{input}`");
        }
    }

    #[test]
    fn normalized_methods_exist_on_every_type() {
        assert_eq!(IriBuf::new("HTTP://A/./b").unwrap().normalized(), "http://a/b");
        assert_eq!(IriBuf::new("HTTP://A:80").unwrap().normalized_for_scheme().as_str(), "http://a/");
        assert_eq!(UriBuf::new("HTTP://A/%7e").unwrap().normalized().as_str(), "http://a/~");
        assert_eq!(UriRefBuf::new("./%7E").unwrap().normalized().as_str(), "~");
        assert_eq!(IriRef::parse("A/../é").unwrap().normalized().as_str(), "é");
    }

    #[test]
    fn equivalent_exists_on_every_type_and_accepts_any_storage() {
        assert!(IriBuf::new("HTTP://A/b").unwrap().equivalent(&Iri::parse("http://a/b").unwrap()));
        assert!(Iri::parse("http://a/%7e").unwrap().equivalent(&IriBuf::new("http://a/~").unwrap()));
        assert!(IriRefBuf::new("a/../../b").unwrap().equivalent(&IriRef::parse("../b").unwrap()));
        assert!(UriBuf::new("HTTP://A/b").unwrap().equivalent(&Uri::parse("http://a/b").unwrap()));
        assert!(UriRefBuf::new("./%7E").unwrap().equivalent(&UriRef::parse("~").unwrap()));
    }

    /// Spellings that RFC 3986 §6.2.2 makes equivalent are still different
    /// strings, so §5.3.1 `==` tells them apart.
    #[test]
    fn syntax_based_equivalents_are_equivalent_but_not_equal() {
        for (a, b) in [
            ("HTTP://A/b", "http://a/b"),
            ("http://a/%7e", "http://a/~"),
            ("http://a/%2f", "http://a/%2F"),
            ("foo:a/../../b", "foo:/b"),
            ("a/../../b", "../b"),
        ] {
            let (x, y) = (IriRefBuf::new(a).unwrap(), IriRefBuf::new(b).unwrap());
            assert!(x.equivalent(&y), "`{a}` should be equivalent to `{b}`");
            assert!(y.equivalent(&x), "`{b}` should be equivalent to `{a}`");
            assert_ne!(x, y, "`{a}` should not be `==` to `{b}`");
        }
    }

    /// Scheme-based defaults, trailing slashes and path case are outside
    /// §6.2.2, so these are neither equal nor equivalent.
    #[test]
    fn scheme_based_and_case_differences_are_neither_equal_nor_equivalent() {
        for (a, b) in [("http://a:80/", "http://a/"), ("/foo/bar", "/foo/bar/"), ("http://a/A", "http://a/a")] {
            let (x, y) = (IriRefBuf::new(a).unwrap(), IriRefBuf::new(b).unwrap());
            assert!(!x.equivalent(&y), "`{a}` should not be equivalent to `{b}`");
            assert_ne!(x, y, "`{a}` should not be `==` to `{b}`");
        }
    }

    const CORPUS: &[&str] = &[
        "http://a/b",
        "HTTP://A/b",
        "http://a/%7Eb",
        "http://a/%7eb",
        "http://a/~b",
        "http://a/%2f",
        "http://a/%2F",
        "http://a/b/../c",
        "http://a/c",
        "http://a/b/%2E%2E/c",
        "http://a/b/%2e%2E/c/",
        "http://exa%6Dple.org/",
        "http://EXAMPLE.org/",
        "http://example.org/",
        "http://U%7e@h/",
        "http://U~@h/",
        "http://u~@h/",
        "http://a:80/",
        "http://a/",
        "http://a",
        "http://host//a",
        "http://host/b/..//a",
        "http:/.//a",
        "http:/b/..//a",
        "urn:a:b",
        "urn:./a:b",
        "../a",
        "a",
        "./a",
        "..",
        "",
        "../..",
        "../../",
        "x/../a:b",
        "./a:b",
        ".",
        "./",
        "a/..",
        "%2E",
        ".?q",
        "./?q",
        "?q",
        "foo:a/..",
        "foo:",
        "foo:.",
        "http://a/?Q=%7e",
        "http://a/?Q=~",
        "http://a/?q=~",
        "http://a/#%7E",
        "http://a/#~",
        "http://[::A]/",
        "http://[::a]/",
        "http://A%c3%a9/",
        "http://a%C3%A9/",
        "http://a/%ff",
        "http://a/%FF",
        "http://a/%c3x",
        "http://a/%C3x",
        "http://é.org/%C3%A9",
        "http://É.org/%c3%a9",
    ];

    /// Two references are equivalent exactly when their normal forms are
    /// byte-equal, every reference is equivalent to its normal form, and
    /// normalizing is idempotent.
    #[test]
    fn equivalent_agrees_with_normalized() {
        for &a in CORPUS {
            let (x, na) = (IriRefBuf::new(a).unwrap(), normalized(a));
            assert!(x.equivalent(&IriRefBuf::new(na.clone()).unwrap()), "`{a}` should be equivalent to `{na}`");
            assert_eq!(normalized(&na), na, "normalizing `{a}` is not idempotent");
            for &b in CORPUS {
                let (y, nb) = (IriRefBuf::new(b).unwrap(), normalized(b));
                assert_eq!(x.equivalent(&y), na == nb, "`{a}` vs `{b}` disagrees with `{na}` vs `{nb}`");
            }
        }
    }

    fn hash_of<V: Hash + ?Sized>(hasher: &RandomState, value: &V) -> u64 {
        hasher.hash_one(value)
    }

    /// `==`, `Hash` and `Ord` are RFC 3987 §5.3.1 Simple String Comparison on
    /// all four types, so they agree with `as_str()` for every pair, and a
    /// value hashes as its `str` does, as `Borrow<str>` requires.
    fn check_string_identity<V: Eq + Ord + Hash + Deref<Target = str>>(values: &[V]) {
        let hasher = RandomState::new();
        for a in values {
            assert_eq!(hash_of(&hasher, a), hash_of(&hasher, &**a), "`{}`", &**a);
            for b in values {
                assert_eq!(a == b, **a == **b, "`{}` == `{}`", &**a, &**b);
                assert_eq!(a.cmp(b), (**a).cmp(&**b), "`{}` cmp `{}`", &**a, &**b);
                if a == b {
                    assert_eq!(hash_of(&hasher, a), hash_of(&hasher, b), "`{}`", &**a);
                }
            }
        }
        let distinct: HashSet<&str> = values.iter().map(|v| &**v).collect();
        assert_eq!(values.iter().collect::<HashSet<_>>().len(), distinct.len());
        assert_eq!(values.iter().collect::<BTreeSet<_>>().len(), distinct.len());
    }

    #[test]
    fn eq_hash_and_ord_compare_as_str_on_every_type() {
        let iri_refs: Vec<IriRefBuf> = CORPUS.iter().map(|s| IriRefBuf::new(*s).unwrap()).collect();
        let iris: Vec<IriBuf> = CORPUS.iter().filter_map(|s| IriBuf::new(*s).ok()).collect();
        let uri_refs: Vec<UriRefBuf> = CORPUS.iter().filter_map(|s| UriRefBuf::new(*s).ok()).collect();
        let uris: Vec<UriBuf> = CORPUS.iter().filter_map(|s| UriBuf::new(*s).ok()).collect();
        assert_eq!(iri_refs.len(), CORPUS.len());
        assert!(!iris.is_empty() && !uri_refs.is_empty() && !uris.is_empty());
        check_string_identity(&iri_refs);
        check_string_identity(&iris);
        check_string_identity(&uri_refs);
        check_string_identity(&uris);
        assert_eq!(
            IriBuf::new("HTTP://A/b").unwrap().cmp(&IriBuf::new("http://a/b").unwrap()),
            Ordering::Less,
            "`Ord` is byte order: `H` sorts before `h`"
        );
    }
}
