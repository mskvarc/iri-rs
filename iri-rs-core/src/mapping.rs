//! RFC 3987 §3.1 mapping of IRIs to URIs.
//!
//! Every non-ASCII character is replaced by the percent-encoded octets of its
//! UTF-8 form, with uppercase hex digits. ASCII is copied as it is, escapes
//! included, so a `%` already in the IRI is never encoded again.
//!
//! `to_uri()` percent-encodes the host like every other component, the mapping
//! §3.1 defines for every IRI. §3.1 also allows `ToASCII` for a host known to be
//! a DNS name; with the `idna` feature, `to_uri_with_idna_host()` does that. It
//! is a method of its own, so enabling the feature never changes `to_uri()`.
//!
//! The mapping is what RFC 3987 §5.3.2.3 aligns percent-encoding with:
//! `http://a/%C3%A9` and `http://a/é` are not `equivalent()` as IRIs, while
//! their URIs are.

use std::{borrow::Cow, ops::Deref};

#[cfg(feature = "idna")]
use std::ops::Range;

#[cfg(feature = "idna")]
use idna::AsciiDenyList;
#[cfg(feature = "idna")]
use pct::PctStr;

#[cfg(feature = "idna")]
use crate::components::split_authority;
use crate::{
    parse::Positions,
    types::{Iri, IriRef, Uri, UriRef},
};

/// Hex digits of a percent-encoded octet, uppercase as RFC 3986 §2.1 prefers.
const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Appends `s` to `out` with each non-ASCII character percent-encoded.
fn push_mapped(out: &mut String, s: &str) {
    for c in s.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut octets = [0; 4];
            for &b in c.encode_utf8(&mut octets).as_bytes() {
                out.push('%');
                out.push(char::from(HEX[usize::from(b >> 4)]));
                out.push(char::from(HEX[usize::from(b & 0xF)]));
            }
        }
    }
}

/// The length `s` has once mapped.
fn mapped_len(s: &str) -> usize {
    s.len() + s.bytes().filter(|b| !b.is_ascii()).count() * 2
}

/// `iri` with each non-ASCII character percent-encoded, or `None` when it is
/// all ASCII and maps to itself.
fn map_non_ascii(iri: &str) -> Option<String> {
    if iri.is_ascii() {
        return None;
    }
    let mut out = String::with_capacity(mapped_len(iri));
    push_mapped(&mut out, iri);
    Some(out)
}

/// The byte range of the host of `iri` and the host UTS 46 `ToASCII` maps it
/// to, when the host is a reg-name holding non-ASCII characters, written or
/// percent-encoded, and `ToASCII` accepts it.
///
/// Escapes are decoded first, as `ToASCII` reads characters: `b%C3%BCcher` and
/// `bücher` are the same name. Characters a URL host may not hold (`/`, `@`,
/// `%` and the like, the WHATWG forbidden domain code points) fail `ToASCII`.
#[cfg(feature = "idna")]
fn ascii_host(iri: &str, positions: Positions) -> Option<(Range<usize>, String)> {
    let authority_start = positions.scheme_end + 2;
    if authority_start > positions.authority_end {
        return None;
    }
    let (user_info, host, _) = split_authority(&iri[authority_start..positions.authority_end]);
    if host.starts_with('[') {
        return None;
    }
    let decoded: Vec<u8> = PctStr::new(host).ok()?.bytes().collect();
    if decoded.is_ascii() {
        return None;
    }
    let ascii = idna::domain_to_ascii_cow(&decoded, AsciiDenyList::URL).ok()?.into_owned();
    let start = authority_start + user_info.map_or(0, |user_info| user_info.len() + 1);
    Some((start..start + host.len(), ascii))
}

/// `iri` mapped with its host through `ToASCII` where [`ascii_host`] maps it,
/// or `None` when it is all ASCII and maps to itself.
#[cfg(feature = "idna")]
fn map_with_idna_host(iri: &str, positions: Positions) -> Option<String> {
    let Some((host, ascii)) = ascii_host(iri, positions) else {
        return map_non_ascii(iri);
    };
    let (before, after) = (&iri[..host.start], &iri[host.end..]);
    let mut out = String::with_capacity(mapped_len(before) + ascii.len() + mapped_len(after));
    push_mapped(&mut out, before);
    out.push_str(&ascii);
    push_mapped(&mut out, after);
    Some(out)
}

/// The URI reference a mapping produced, or `iri` itself when it had nothing
/// to map.
fn uri_ref_of(iri: &str, positions: Positions, mapped: Option<String>) -> UriRef<Cow<'_, str>> {
    match mapped {
        Some(uri) => UriRef::parse_unchecked(Cow::Owned(uri)),
        None => UriRef::from_raw_parts(Cow::Borrowed(iri), positions),
    }
}

/// The URI a mapping produced, or `iri` itself when it had nothing to map.
fn uri_of(iri: &str, positions: Positions, mapped: Option<String>) -> Uri<Cow<'_, str>> {
    match mapped {
        Some(uri) => Uri::parse_unchecked(Cow::Owned(uri)),
        None => Uri::from_raw_parts(Cow::Borrowed(iri), positions),
    }
}

impl<T: Deref<Target = str>> IriRef<T> {
    /// Returns the URI reference this IRI reference maps to under RFC 3987
    /// §3.1: each non-ASCII character becomes the percent-encoded octets of
    /// its UTF-8 form, uppercase hex. Nothing else changes, escapes included.
    ///
    /// Borrows the input when it is all ASCII, which is then already a URI
    /// reference.
    ///
    /// ```
    /// use iri_rs_core::IriRef;
    ///
    /// let iri = IriRef::parse("caf\u{e9}?q=%7e").unwrap();
    /// assert_eq!(iri.to_uri_ref().as_str(), "caf%C3%A9?q=%7e");
    /// ```
    #[must_use]
    pub fn to_uri_ref(&self) -> UriRef<Cow<'_, str>> {
        uri_ref_of(self.as_str(), self.positions, map_non_ascii(self.as_str()))
    }

    /// Returns [`to_uri_ref`](Self::to_uri_ref), except that a reg-name host
    /// with non-ASCII characters, written or percent-encoded, is mapped by
    /// UTS 46 `ToASCII` (IDNA) instead, as RFC 3987 §3.1 allows for a host known
    /// to be a DNS name. Escapes in the host are decoded first. A host `ToASCII`
    /// refuses is percent-encoded as `to_uri_ref` does.
    ///
    /// Whether the host is a DNS name is the caller's to know: the scheme does
    /// not say.
    ///
    /// ```
    /// use iri_rs_core::IriRef;
    ///
    /// let iri = IriRef::parse("//B\u{fc}cher.example/caf\u{e9}").unwrap();
    /// assert_eq!(iri.to_uri_ref_with_idna_host().as_str(), "//xn--bcher-kva.example/caf%C3%A9");
    /// ```
    #[cfg(feature = "idna")]
    #[must_use]
    pub fn to_uri_ref_with_idna_host(&self) -> UriRef<Cow<'_, str>> {
        uri_ref_of(self.as_str(), self.positions, map_with_idna_host(self.as_str(), self.positions))
    }
}

impl<T: Deref<Target = str>> Iri<T> {
    /// Returns the URI this IRI maps to under RFC 3987 §3.1: each non-ASCII
    /// character becomes the percent-encoded octets of its UTF-8 form,
    /// uppercase hex. Nothing else changes, escapes included.
    ///
    /// Borrows the input when it is all ASCII, which is then already a URI.
    /// Comparing the URIs aligns an escape with the character it encodes, as
    /// RFC 3987 §5.3.2.3 describes; `equivalent()` on the IRIs does not.
    ///
    /// ```
    /// use iri_rs_core::Iri;
    ///
    /// let spelled = Iri::parse("https://example.org/Par\u{ed}s").unwrap();
    /// let escaped = Iri::parse("https://example.org/Par%c3%ads").unwrap();
    /// assert_eq!(spelled.to_uri().as_str(), "https://example.org/Par%C3%ADs");
    /// assert!(!spelled.equivalent(&escaped));
    /// assert!(spelled.to_uri().equivalent(&escaped.to_uri()));
    /// ```
    #[must_use]
    pub fn to_uri(&self) -> Uri<Cow<'_, str>> {
        uri_of(self.as_str(), self.positions(), map_non_ascii(self.as_str()))
    }

    /// Returns [`to_uri`](Self::to_uri), except that a reg-name host with
    /// non-ASCII characters, written or percent-encoded, is mapped by UTS 46
    /// `ToASCII` (IDNA) instead, as RFC 3987 §3.1 allows for a host known to be
    /// a DNS name. Escapes in the host are decoded first. A host `ToASCII`
    /// refuses is percent-encoded as `to_uri` does.
    ///
    /// Whether the host is a DNS name is the caller's to know: the scheme does
    /// not say. For `http` and `https` it is.
    ///
    /// ```
    /// use iri_rs_core::Iri;
    ///
    /// let spelled = Iri::parse("https://B\u{fc}cher.example/caf\u{e9}").unwrap();
    /// let encoded = Iri::parse("https://xn--bcher-kva.example/caf%C3%A9").unwrap();
    /// assert_eq!(spelled.to_uri_with_idna_host().as_str(), encoded.as_str());
    /// ```
    #[cfg(feature = "idna")]
    #[must_use]
    pub fn to_uri_with_idna_host(&self) -> Uri<Cow<'_, str>> {
        uri_of(self.as_str(), self.positions(), map_with_idna_host(self.as_str(), self.positions()))
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::types::{Iri, IriRef, Uri, UriRef};

    fn mapped(iri: &str) -> String {
        Iri::parse(iri).unwrap().to_uri().as_str().to_owned()
    }

    #[test]
    fn ascii_iri_maps_to_itself_borrowed() {
        let iri = Iri::parse("http://a/%7e/b?c#d").unwrap();
        let uri = iri.to_uri();
        assert_eq!(uri.as_str(), "http://a/%7e/b?c#d");
        assert!(matches!(uri.into_inner(), Cow::Borrowed(_)));
    }

    #[test]
    fn non_ascii_characters_are_utf8_percent_encoded_in_uppercase() {
        assert_eq!(mapped("https://example.org/Par\u{ed}s"), "https://example.org/Par%C3%ADs");
        assert_eq!(mapped("http://a/\u{1f600}"), "http://a/%F0%9F%98%80");
        assert_eq!(mapped("http://a/\u{4e2d}\u{6587}"), "http://a/%E4%B8%AD%E6%96%87");
    }

    #[test]
    fn existing_escapes_are_kept_as_written() {
        assert_eq!(mapped("http://a/%7e%2f/\u{e9}"), "http://a/%7e%2f/%C3%A9");
    }

    #[test]
    fn every_component_is_mapped() {
        let iri = Iri::parse("http://\u{fc}ser@b\u{fc}cher.example/p\u{e5}th?q\u{e9}=\u{e9}&\u{e000}#fr\u{e4}g").unwrap();
        let uri = iri.to_uri();
        assert_eq!(uri.authority(), Some("%C3%BCser@b%C3%BCcher.example"));
        assert_eq!(uri.path(), "/p%C3%A5th");
        assert_eq!(uri.query(), Some("q%C3%A9=%C3%A9&%EE%80%80"));
        assert_eq!(uri.fragment(), Some("fr%C3%A4g"));
    }

    #[test]
    fn the_mapping_is_a_valid_uri_with_its_own_positions() {
        for iri in ["https://example.org/Par\u{ed}s?\u{e9}#\u{e9}", "foo:\u{e9}", "http://[::1]/\u{e9}"] {
            let iri = Iri::parse(iri).unwrap();
            let uri = iri.to_uri();
            let reparsed = Uri::parse(uri.as_str()).unwrap();
            assert_eq!(uri.positions(), reparsed.positions(), "{iri}");
        }
    }

    #[test]
    fn a_relative_reference_maps_to_a_uri_reference() {
        let iri = IriRef::parse("../caf\u{e9}?\u{e9}").unwrap();
        let uri = iri.to_uri_ref();
        assert_eq!(uri.as_str(), "../caf%C3%A9?%C3%A9");
        assert!(UriRef::parse(uri.as_str()).is_ok());
        assert!(matches!(IriRef::parse("a/b").unwrap().to_uri_ref().into_inner(), Cow::Borrowed("a/b")));
    }

    #[test]
    fn mapping_aligns_an_escape_with_the_character_it_encodes() {
        let spelled = Iri::parse("HTTP://Example.org/Par\u{ed}s").unwrap();
        let escaped = Iri::parse("http://example.org/Par%c3%ads").unwrap();
        assert!(!spelled.equivalent(&escaped));
        assert!(spelled.to_uri().equivalent(&escaped.to_uri()));
        assert_eq!(spelled.to_uri().normalized(), escaped.to_uri().normalized());
    }

    #[cfg(feature = "idna")]
    mod idna_host {
        use std::borrow::Cow;

        use crate::types::{Iri, IriRef, Uri};

        fn mapped(iri: &str) -> String {
            Iri::parse(iri).unwrap().to_uri_with_idna_host().as_str().to_owned()
        }

        #[test]
        fn a_non_ascii_host_is_mapped_by_to_ascii_and_the_rest_percent_encoded() {
            assert_eq!(
                mapped("https://\u{fc}ser@B\u{fc}cher.example:8080/caf\u{e9}?\u{e9}#\u{e9}"),
                "https://%C3%BCser@xn--bcher-kva.example:8080/caf%C3%A9?%C3%A9#%C3%A9"
            );
            assert_eq!(mapped("http://\u{4f8b}\u{3048}.\u{30c6}\u{30b9}\u{30c8}/"), "http://xn--r8jz45g.xn--zckzah/");
        }

        #[test]
        fn a_percent_encoded_non_ascii_host_is_decoded_before_to_ascii() {
            assert_eq!(mapped("https://b%C3%BCcher.example/"), "https://xn--bcher-kva.example/");
            assert_eq!(mapped("https://b%c3%bccher.example/"), mapped("https://b\u{fc}cher.example/"));
        }

        #[test]
        fn an_ascii_host_is_left_as_written() {
            assert_eq!(mapped("https://Example.ORG/caf\u{e9}"), "https://Example.ORG/caf%C3%A9");
            assert_eq!(mapped("https://%61.example/"), "https://%61.example/");
            let iri = Iri::parse("https://example.org/a").unwrap();
            assert!(matches!(iri.to_uri_with_idna_host().into_inner(), Cow::Borrowed(_)));
        }

        #[test]
        fn an_ip_literal_host_is_left_as_written() {
            assert_eq!(mapped("http://[::1]/caf\u{e9}"), "http://[::1]/caf%C3%A9");
        }

        #[test]
        fn a_host_to_ascii_refuses_is_percent_encoded() {
            assert_eq!(mapped("http://b\u{fc}cher%2Fx.example/"), "http://b%C3%BCcher%2Fx.example/");
            assert_eq!(mapped("http://b\u{fc}cher%40x.example/"), "http://b%C3%BCcher%40x.example/");
        }

        #[test]
        fn the_mapping_is_a_valid_uri_with_its_own_positions() {
            for iri in ["https://\u{fc}@b\u{fc}cher.example:1/\u{e9}?\u{e9}#\u{e9}", "foo://b\u{fc}cher/x"] {
                let iri = Iri::parse(iri).unwrap();
                let uri = iri.to_uri_with_idna_host();
                assert_eq!(uri.positions(), Uri::parse(uri.as_str()).unwrap().positions(), "{iri}");
            }
        }

        #[test]
        fn a_relative_reference_with_an_authority_maps_its_host() {
            let iri = IriRef::parse("//b\u{fc}cher.example/caf\u{e9}").unwrap();
            assert_eq!(iri.to_uri_ref_with_idna_host().as_str(), "//xn--bcher-kva.example/caf%C3%A9");
            let path_only = IriRef::parse("caf\u{e9}").unwrap();
            assert_eq!(path_only.to_uri_ref_with_idna_host().as_str(), "caf%C3%A9");
        }
    }
}
