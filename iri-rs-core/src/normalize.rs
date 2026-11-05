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

use std::{borrow::Cow, cmp::Ordering, hash::Hasher, ops::Deref};

use pct::rfc3986::{Chunk, chunks};

use crate::{
    components::{normalize_path, path_guard, path_is_normalized, split_authority},
    parse::Positions,
    resolve::remove_dot_segments,
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
        .then_with(|| path_cmp(x.path, y.path, x.scheme.is_none() && x.authority.is_none()))
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

/// `relative` says that both paths belong to relative-path references. The
/// caller only gets here when the schemes and authorities compared equal, so
/// it holds for both or neither.
fn path_cmp(a: &str, b: &str, relative: bool) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    canonical_path(a, relative).cmp(&canonical_path(b, relative))
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
    path_cmp(a, b, false) == Ordering::Equal
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
/// With `scheme_defaults`, the RFC 3986 §6.2.3 scheme-based steps are applied
/// too: an empty port is dropped for every scheme, and for `http` and `https`
/// the default port is dropped and an empty path under an authority becomes
/// `/`. Equality and hashing never use them.
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
        if let Some(port) = port.filter(|&port| !(scheme_defaults && port.is_empty()) && Some(port) != default_port) {
            sink.put(":");
            sink.put(port);
        }
    }
    let path = canonical_path(&iri[p.authority_end..p.path_end], scheme.is_none() && !has_authority);
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
            /// §6.2.3 scheme-based steps applied on top: an empty port (`a:`)
            /// is dropped, and for `http` and `https` so is the default port
            /// (`:80`, `:443`), and an empty path under an authority becomes
            /// `/`. Other schemes keep everything else as `normalized` leaves
            /// it.
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

impl<T: Deref<Target = str>> Iri<T> {
    /// The normal form `==` compares, borrowed when `self` is visibly in it,
    /// or `None` under `bytewise`, where `==` compares `as_str()` itself.
    ///
    /// For `iri-rs-enum`, whose derive matches `as_str()` against the declared
    /// spellings of its variants, and on a miss matches this against their
    /// normal forms.
    #[doc(hidden)]
    #[must_use]
    pub fn __eq_form(&self) -> Option<Cow<'_, str>> {
        let (s, p) = (&*self.0.iri, self.0.positions);
        if cfg!(feature = "bytewise") {
            None
        } else if is_plainly_normal(s, p) {
            Some(Cow::Borrowed(s))
        } else {
            Some(Cow::Owned(normalized_string(s, p, false)))
        }
    }
}

/// Whether `s` is visibly its own normal form: no escapes, no ASCII
/// uppercase in the scheme or host, no dot segments. Never says so of a string
/// that normalization changes; it may miss some that it leaves alone.
fn is_plainly_normal(s: &str, p: Positions) -> bool {
    if memchr::memchr(b'%', s.as_bytes()).is_some() {
        return false;
    }
    let parts = Parts::of(s, p);
    let has_upper = |t: &str| t.bytes().any(|b| b.is_ascii_uppercase());
    if parts.scheme.is_some_and(has_upper) {
        return false;
    }
    if parts.authority.is_some_and(|a| has_upper(split_authority(a).1)) {
        return false;
    }
    // A relative-path reference can also gain a `./`, but only through dot
    // segments, which this already rules out.
    path_is_normalized(parts.path)
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

    /// A relative-path reference whose path dot removal empties still names
    /// the base's directory, not the base, so it keeps a `./`.
    #[test]
    fn relative_references_naming_the_base_directory_keep_dot_slash() {
        for a in [".", "./", "a/..", "%2E", "a/b/../..", "./."] {
            assert_eq!(normalized(a), "./", "`{a}`");
            assert!(!eq(a, ""), "`{a}` should not equal the empty reference");
            assert!(eq(a, "./"), "`{a}` should equal `./`");
        }
        assert_eq!(normalized(".?q"), "./?q");
        assert!(!eq(".?q", "?q"));
        // With a scheme the path is not merged with a base: dot segments go as
        // resolution removes them.
        assert_eq!(normalized("foo:a/.."), "foo:/");
        assert!(eq("foo:.", "foo:"));
    }

    /// Under a scheme, dot segments go exactly as resolution removes them
    /// (RFC 3986 §5.2.4, not Errata 4547), so equal IRIs resolve alike.
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

    /// Every IRI with a scheme equals what it resolves to: resolution applies
    /// the same dot-segment removal as the normal form.
    #[test]
    fn an_iri_equals_its_resolution() {
        const SEGMENTS: &[&str] = &["", "a", "b", ".", "..", "%2E", "%2e%2E", "a:b"];
        use crate::types::{IriBuf, IriRefBuf};
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
                assert!(eq(&s, buf.as_str()), "`{s}` should equal its resolution `{}`", buf.as_str());
            }
            assert_eq!(decoded.as_str(), normalized(&s), "`{s}`");
        }
    }

    /// Normalizing a reference never changes what it resolves to.
    ///
    /// Left out: a rootless base with no `/` (`foo:`). Merging with it leaves
    /// the reference's path as it is, and §5.2.4 gives `a/..` a leading `/`
    /// there but `./` none. Equality cannot depend on the base, so `a/..` and
    /// `./` stay equal, as they are against every other base.
    #[test]
    fn normalizing_a_reference_keeps_its_resolution() {
        use crate::types::{IriBuf, IriRefBuf};
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
                // Through `iri_eq`, which `bytewise` does not swap out.
                assert!(
                    eq(plain.as_str(), norm.as_str()),
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

    fn hash_with<H: Hasher + Default>(s: &str) -> u64 {
        let mut hasher = H::default();
        normalized_hash(s, find_iri_ref_positions(s), &mut hasher);
        hasher.finish()
    }

    /// `is_plainly_normal` may only vouch for strings normalization leaves
    /// alone, or the enum derive would miss variants.
    #[test]
    fn plainly_normal_is_never_optimistic() {
        for &s in CORPUS {
            let p = find_iri_ref_positions(s);
            if is_plainly_normal(s, p) {
                assert_eq!(normalized(s), s, "`{s}` was vouched for");
            }
        }
        for s in ["http://a/b", "foo:a:b", "http://u:P@a/B?Q#F"] {
            assert!(is_plainly_normal(s, find_iri_ref_positions(s)), "`{s}`");
        }
    }

    #[test]
    fn eq_form_is_the_normal_form() {
        let form = |s: &str| IriBuf::new(s).unwrap().__eq_form().map(Cow::into_owned);
        if cfg!(feature = "bytewise") {
            assert_eq!(form("HTTPS://A/b"), None);
        } else {
            assert!(matches!(IriBuf::new("http://a/b").unwrap().__eq_form(), Some(Cow::Borrowed("http://a/b"))));
            assert_eq!(form("http://a/%2F").as_deref(), Some("http://a/%2F"));
            assert_eq!(form("HTTPS://A/%7e/./b").as_deref(), Some("https://a/~/b"));
        }
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
