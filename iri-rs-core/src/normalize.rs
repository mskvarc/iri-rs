//! RFC 3986/3987 §6.2.2 syntax-based normalization: the normal form, and the
//! streaming comparators and hash that agree with it.
//!
//! The normal form is defined once, by `write_normalized`. `normalized()`
//! writes it into a string and [`normalized_hash`] feeds it to the hasher, so
//! the two cannot drift. [`iri_eq`] compares the same canonical components
//! without building the whole form, and the tests hold it to `normalized()`.
//!
//! Per component:
//! * scheme: ASCII-lowercased;
//! * user info: percent-normalized, case kept;
//! * host: percent-normalized, then ASCII-lowercased outside the remaining
//!   `%XX` triplets;
//! * port: kept as written;
//! * path: percent-normalized, then dot segments removed by
//!   [`normalize_path`];
//! * query, fragment: percent-normalized, case and order kept.
//!
//! Percent-normalization decodes escaped unreserved characters (letters,
//! digits, `-` `.` `_` `~`) and uppercases the hex digits of every other
//! escape. It runs before dot-segment removal, so `%2E%2E` is a `..` segment.

use std::{borrow::Cow, cmp::Ordering, hash::Hasher, ops::Deref};

use pct::rfc3986::{Chunk, chunks};

use crate::{
    components::{normalize_path, path_guard, path_is_normalized, split_authority},
    parse::Positions,
    types::{Iri, IriRef, Uri, UriRef},
};

/// The components of an IRI reference, as equality and ordering see them.
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

/// Whether two IRI references have the same normal form.
#[must_use]
pub fn iri_eq(a: &str, pa: Positions, b: &str, pb: Positions) -> bool {
    iri_cmp(a, pa, b, pb) == Ordering::Equal
}

/// Orders IRI references by their normal form, component by component: the
/// scheme, then the user info, host and port, then the path, query and
/// fragment. A missing component sorts before a present one.
///
/// This is the single definition of equivalence: [`iri_eq`] is
/// `iri_cmp(..) == Equal`, so ordering and equality cannot disagree.
#[must_use]
pub fn iri_cmp(a: &str, pa: Positions, b: &str, pb: Positions) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (x, y) = (Parts::of(a, pa), Parts::of(b, pb));
    opt_cmp(x.scheme, y.scheme, lower_cmp)
        .then_with(|| opt_cmp(x.authority, y.authority, authority_cmp))
        .then_with(|| path_cmp(x.path, y.path))
        .then_with(|| opt_cmp(x.query, y.query, pct_cmp))
        .then_with(|| opt_cmp(x.fragment, y.fragment, pct_cmp))
}

fn opt_cmp(a: Option<&str>, b: Option<&str>, cmp: impl FnOnce(&str, &str) -> Ordering) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => cmp(x, y),
        _ => a.is_some().cmp(&b.is_some()),
    }
}

fn lower_cmp(a: &str, b: &str) -> Ordering {
    if a.eq_ignore_ascii_case(b) {
        return Ordering::Equal;
    }
    a.bytes().map(|c| c.to_ascii_lowercase()).cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

fn authority_cmp(a: &str, b: &str) -> Ordering {
    let (ui_a, host_a, port_a) = split_authority(a);
    let (ui_b, host_b, port_b) = split_authority(b);
    opt_cmp(ui_a, ui_b, pct_cmp)
        .then_with(|| host_cmp(host_a, host_b))
        .then_with(|| port_a.cmp(&port_b))
}

fn host_cmp(a: &str, b: &str) -> Ordering {
    if memchr::memchr(b'%', a.as_bytes()).is_none() && memchr::memchr(b'%', b.as_bytes()).is_none() {
        return lower_cmp(a, b);
    }
    canonical(a, true).cmp(&canonical(b, true))
}

fn path_cmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    canonical_path(a).cmp(&canonical_path(b))
}

fn pct_cmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    canonical(a, false).cmp(&canonical(b, false))
}

#[must_use]
pub fn authority_eq(a: &str, b: &str) -> bool {
    authority_cmp(a, b) == Ordering::Equal
}

#[must_use]
pub fn path_eq_normalized(a: &str, b: &str) -> bool {
    path_cmp(a, b) == Ordering::Equal
}

#[must_use]
pub fn pct_unreserved_eq(a: &str, b: &str) -> bool {
    pct_cmp(a, b) == Ordering::Equal
}

/// Hashes the normal form, so IRIs that compare equal hash alike.
pub fn normalized_hash<H: Hasher>(iri: &str, p: Positions, state: &mut H) {
    let mut sink = HashSink {
        state,
        buf: [0; HASH_CHUNK],
        len: 0,
    };
    write_normalized(iri, p, false, &mut sink);
    sink.finish();
}

/// Where [`write_normalized`] puts the normal form.
trait Sink {
    fn put(&mut self, s: &str);
    /// Puts `s` with its ASCII letters lowercased.
    fn put_lower(&mut self, s: &str);
    /// Puts one ASCII byte.
    fn put_ascii(&mut self, byte: u8);
}

impl Sink for String {
    fn put(&mut self, s: &str) {
        self.push_str(s);
    }

    fn put_lower(&mut self, s: &str) {
        let start = self.len();
        self.push_str(s);
        self[start..].make_ascii_lowercase();
    }

    fn put_ascii(&mut self, byte: u8) {
        debug_assert!(byte.is_ascii());
        self.push(char::from(byte));
    }
}

const HASH_CHUNK: usize = 128;

/// Feeds a hasher the normal form in fixed-size chunks.
///
/// `Hasher::write` is not required to be streaming: hashers such as `FxHasher`
/// mix each call separately, so `write(b"a~")` and `write(b"a"); write(b"~")`
/// differ. Equal IRIs must therefore reach the hasher as identical calls. The
/// chunk boundaries depend only on the position in the normal form, never on
/// how the input spelled it.
struct HashSink<'a, H> {
    state: &'a mut H,
    buf: [u8; HASH_CHUNK],
    len: usize,
}

impl<H: Hasher> HashSink<'_, H> {
    fn put_bytes(&mut self, mut bytes: &[u8], lower: bool) {
        while !bytes.is_empty() {
            let n = (HASH_CHUNK - self.len).min(bytes.len());
            let dst = &mut self.buf[self.len..self.len + n];
            dst.copy_from_slice(&bytes[..n]);
            if lower {
                dst.make_ascii_lowercase();
            }
            self.len += n;
            bytes = &bytes[n..];
            if self.len == HASH_CHUNK {
                self.state.write(&self.buf);
                self.len = 0;
            }
        }
    }

    fn finish(self) {
        if self.len > 0 {
            self.state.write(&self.buf[..self.len]);
        }
    }
}

impl<H: Hasher> Sink for HashSink<'_, H> {
    fn put(&mut self, s: &str) {
        self.put_bytes(s.as_bytes(), false);
    }

    fn put_lower(&mut self, s: &str) {
        self.put_bytes(s.as_bytes(), true);
    }

    fn put_ascii(&mut self, byte: u8) {
        self.buf[self.len] = byte;
        self.len += 1;
        if self.len == HASH_CHUNK {
            self.state.write(&self.buf);
            self.len = 0;
        }
    }
}

/// Writes the normal form of the IRI reference `iri` (see the module docs).
///
/// With `scheme_defaults`, the RFC 3986 §6.2.3 scheme-based steps for `http`
/// and `https` are applied too: the default port is dropped and an empty path
/// under an authority becomes `/`. Equality and hashing never use them.
fn write_normalized<S: Sink>(iri: &str, p: Positions, scheme_defaults: bool, sink: &mut S) {
    let scheme = (p.scheme_end > 0).then(|| &iri[..p.scheme_end - 1]);
    if let Some(scheme) = scheme {
        sink.put_lower(scheme);
        sink.put(":");
    }
    let default_port = match scheme {
        Some(s) if scheme_defaults && s.eq_ignore_ascii_case("http") => Some("80"),
        Some(s) if scheme_defaults && s.eq_ignore_ascii_case("https") => Some("443"),
        _ => None,
    };
    let has_authority = p.authority_end > p.scheme_end;
    if has_authority {
        sink.put("//");
        let (user_info, host, port) = split_authority(&iri[p.scheme_end + 2..p.authority_end]);
        if let Some(user_info) = user_info {
            put_pct_normalized(sink, user_info, false);
            sink.put("@");
        }
        put_pct_normalized(sink, host, true);
        if let Some(port) = port.filter(|&port| Some(port) != default_port) {
            sink.put(":");
            sink.put(port);
        }
    }
    let path = canonical_path(&iri[p.authority_end..p.path_end]);
    if path.is_empty() && has_authority && default_port.is_some() {
        sink.put("/");
    } else {
        sink.put(path_guard(scheme.is_some(), has_authority, &path));
        sink.put(&path);
    }
    if p.query_end > p.path_end {
        sink.put("?");
        put_pct_normalized(sink, &iri[p.path_end + 1..p.query_end], false);
    }
    if iri.len() > p.query_end {
        sink.put("#");
        put_pct_normalized(sink, &iri[p.query_end + 1..], false);
    }
}

/// Puts `s` percent-normalized. With `lower`, ASCII letters are lowercased
/// too, except the hex digits of the escapes that stay encoded.
///
/// How an escape normalizes is [`pct::rfc3986`]'s decision. It needs no
/// UTF-8 from the decoded octets (`%FF` is a valid escape), and encodes a `%`
/// that starts no escape, which only an unchecked parse can produce, as `%25`.
/// Equality, hashing and `normalized()` all go through here.
fn put_pct_normalized<S: Sink>(sink: &mut S, s: &str, lower: bool) {
    for chunk in chunks(s) {
        match chunk {
            Chunk::Literal(run) if lower => sink.put_lower(run),
            Chunk::Literal(run) => sink.put(run),
            Chunk::Decoded(byte) => sink.put_ascii(if lower { byte.to_ascii_lowercase() } else { byte }),
            Chunk::Encoded(escape) => {
                for byte in escape {
                    sink.put_ascii(byte);
                }
            }
        }
    }
}

/// `s` percent-normalized, borrowed when that changes nothing.
fn canonical(s: &str, lower: bool) -> Cow<'_, str> {
    if memchr::memchr(b'%', s.as_bytes()).is_none() && !(lower && s.bytes().any(|b| b.is_ascii_uppercase())) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    put_pct_normalized(&mut out, s, lower);
    Cow::Owned(out)
}

/// The path percent-normalized and then stripped of dot segments.
fn canonical_path(path: &str) -> Cow<'_, str> {
    let decoded = canonical(path, false);
    if path_is_normalized(&decoded) {
        decoded
    } else {
        Cow::Owned(normalize_path(&decoded))
    }
}

fn normalized_string(iri: &str, p: Positions, scheme_defaults: bool) -> String {
    let mut out = String::with_capacity(iri.len());
    write_normalized(iri, p, scheme_defaults, &mut out);
    out
}

/// Generates `normalized` and `normalized_for_scheme` for one of the four
/// reference types. The output is ASCII whenever the input is, so the URI
/// types stay URIs.
macro_rules! normalized_methods {
    ($ty:ident, $field:ident $(. $inner:tt)?) => {
        impl<T: Deref<Target = str>> $ty<T> {
            /// Returns the RFC 3986/3987 §6.2.2 syntax-based normal form: the
            /// scheme and host lowercased, escaped unreserved characters
            /// decoded, other escapes in uppercase hex, and `.` and `..`
            /// segments removed.
            ///
            /// This is the form equality and hashing compare, so the result is
            /// equal to `self` and hashes alike (unless the `bytewise` feature
            /// makes them byte-wise). The case of the user
            /// info, path, query and fragment is kept, query parameters keep
            /// their order, and no trailing slash is added or removed.
            #[must_use]
            pub fn normalized(&self) -> $ty<String> {
                let s = normalized_string(&self$(.$inner)?.$field, self$(.$inner)?.positions, false);
                $ty::parse_unchecked(s)
            }

            /// Returns [`normalized`](Self::normalized) with the RFC 3986
            /// §6.2.3 scheme-based steps for `http` and `https` applied on top:
            /// the default port (`:80`, `:443`) is dropped, and an empty path
            /// under an authority becomes `/`. Other schemes are left as
            /// `normalized` leaves them.
            ///
            /// Equality is protocol agnostic, so unlike `normalized` the
            /// result can differ from `self`: `http://a:80` and `http://a/`
            /// are not equal.
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

    use crate::types::{IriBuf, IriRefBuf, UriBuf};

    fn normalized(s: &str) -> String {
        IriRefBuf::new(s).unwrap().normalized().into_inner()
    }

    fn normalized_for_scheme(s: &str) -> String {
        IriRefBuf::new(s).unwrap().normalized_for_scheme().into_inner()
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
        ] {
            assert_eq!(normalized_for_scheme(input), expected, "`{input}`");
        }
    }

    #[test]
    fn normalized_methods_exist_on_every_type() {
        assert_eq!(IriBuf::new("HTTP://A/./b").unwrap().normalized(), "http://a/b");
        assert_eq!(IriBuf::new("HTTP://A:80").unwrap().normalized_for_scheme().as_str(), "http://a/");
        assert_eq!(UriBuf::new("HTTP://A/%7e").unwrap().normalized().as_str(), "http://a/~");
        assert_eq!(crate::types::UriRefBuf::new("./%7E").unwrap().normalized().as_str(), "~");
        assert_eq!(IriRef::parse("A/../é").unwrap().normalized().as_str(), "é");
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

    fn hash_with<H: Hasher + Default>(s: &str) -> u64 {
        let mut hasher = H::default();
        normalized_hash(s, find_iri_ref_positions(s), &mut hasher);
        hasher.finish()
    }

    /// Mixes every `write` call separately, as `FxHasher` does, so chunking
    /// that depends on the input's spelling shows up as differing hashes.
    #[derive(Default)]
    struct CallSensitive(u64);

    impl Hasher for CallSensitive {
        fn finish(&self) -> u64 {
            self.0
        }

        fn write(&mut self, bytes: &[u8]) {
            let mut call = std::hash::DefaultHasher::new();
            call.write(bytes);
            self.0 = self.0.rotate_left(5) ^ call.finish();
        }
    }

    /// `normalized()` is the form equality and hashing compare: every IRI
    /// equals its normal form, two IRIs are equal exactly when their normal
    /// forms are byte-equal, and equal IRIs hash alike under any hasher.
    #[test]
    fn normalized_agrees_with_eq_and_hash() {
        for &a in CORPUS {
            let na = normalized(a);
            assert!(eq(a, &na), "`{a}` should equal its normal form `{na}`");
            assert_eq!(normalized(&na), na, "normalizing `{a}` is not idempotent");
            assert_eq!(hash_of(a), hash_of(&na), "`{a}` and `{na}` hash differently");
            for &b in CORPUS {
                let nb = normalized(b);
                assert_eq!(eq(a, b), na == nb, "eq(`{a}`, `{b}`) disagrees with `{na}` vs `{nb}`");
                if eq(a, b) {
                    assert_eq!(hash_of(a), hash_of(b), "`{a}` and `{b}` hash differently");
                    assert_eq!(
                        hash_with::<CallSensitive>(a),
                        hash_with::<CallSensitive>(b),
                        "`{a}` and `{b}` reach the hasher differently"
                    );
                }
            }
        }
    }

    fn cmp(a: &str, b: &str) -> Ordering {
        iri_cmp(a, find_iri_ref_positions(a), b, find_iri_ref_positions(b))
    }

    #[test]
    fn equal_iris_order_as_equal() {
        for (a, b) in [
            ("http://A/", "http://a/"),
            ("http://a/%7E", "http://a/~"),
            ("http://a/b/../c", "http://a/c"),
            ("HTTP://a", "http://a"),
        ] {
            assert_eq!(cmp(a, b), Ordering::Equal, "`{a}` vs `{b}`");
        }
    }

    /// `iri_cmp` is a total order over the corpus, and equality is exactly
    /// its `Equal`, as `Ord` requires.
    #[test]
    fn ordering_is_a_total_order_consistent_with_eq() {
        for &a in CORPUS {
            assert_eq!(cmp(a, a), Ordering::Equal, "`{a}`");
            for &b in CORPUS {
                let ab = cmp(a, b);
                assert_eq!(ab, cmp(b, a).reverse(), "`{a}` vs `{b}` is not antisymmetric");
                assert_eq!(ab == Ordering::Equal, normalized(a) == normalized(b), "`{a}` vs `{b}`");
                for &c in CORPUS {
                    if ab != Ordering::Greater && cmp(b, c) != Ordering::Greater {
                        assert_ne!(cmp(a, c), Ordering::Greater, "`{a}` <= `{b}` <= `{c}` but `{a}` > `{c}`");
                    }
                }
            }
        }
    }

    /// Through the `Ord` impl, which `bytewise` swaps for a byte-wise one.
    #[cfg(not(feature = "bytewise"))]
    #[test]
    fn btree_set_deduplicates_equal_iris() {
        let set: std::collections::BTreeSet<IriRefBuf> = CORPUS.iter().map(|s| IriRefBuf::new(*s).unwrap()).collect();
        let distinct: std::collections::HashSet<String> = CORPUS.iter().map(|s| normalized(s)).collect();
        assert_eq!(set.len(), distinct.len());
    }

    #[test]
    fn long_iris_hash_alike_across_chunk_boundaries() {
        let a = format!("http://a/{}%7E{}", "x".repeat(HASH_CHUNK - 12), "y".repeat(2 * HASH_CHUNK));
        let b = a.replace("%7E", "~");
        assert!(eq(&a, &b));
        assert_eq!(hash_with::<CallSensitive>(&a), hash_with::<CallSensitive>(&b));
    }
}
