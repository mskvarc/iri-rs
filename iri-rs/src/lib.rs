//! This crates provides an implementation of
//! [Uniform Resource Identifiers (URIs, aka URLs)][uri] and [Internationalized
//! Resource Identifiers (IRIs)][iri] following [RFC 3987][uri-rfc] and [RFC
//! 3986][iri-rfc] defined by the [Internet Engineering Task Force
//! (IETF)][ietf] to uniquely identify objects across the web. IRIs are a
//! superclass of URIs accepting international characters defined in the
//! [Unicode][unicode] table.
//!
//! [uri]: <https://en.wikipedia.org/wiki/Uniform_Resource_Identifier>
//! [uri-rfc]: <https://tools.ietf.org/html/rfc3986>
//! [iri]: <https://en.wikipedia.org/wiki/Internationalized_resource_identifier>
//! [iri-rfc]: <https://tools.ietf.org/html/rfc3987>
//! [ietf]: <ietf.org>
//! [unicode]: <https://en.wikipedia.org/wiki/Unicode>
//!
//! URI/IRIs are defined as a sequence of characters with distinguishable
//! components: a scheme, an authority, a path, a query and a fragment.
//!
//! ```text
//!     foo://example.com:8042/over/there?name=ferret#nose
//!     \_/   \______________/\_________/ \_________/ \__/
//!      |           |            |            |        |
//!   scheme     authority       path        query   fragment
//! ```
//!
//! This crate provides types to represent borrowed and owned URIs and IRIs
//! (`Uri`, `Iri`, `UriBuf`, `IriBuf`), borrowed and owned URIs and IRIs
//! references (`UriRef`, `IriRef`, `UriRefBuf`, `IriRefBuf`) and similar
//! types for every part of an URI/IRI. Theses allows the easy access and
//! manipulation of every components.
//! It features:
//!   - borrowed and owned URI/IRIs and URI/IRI-reference;
//!   - mutable URI/IRI buffers (in-place);
//!   - path normalization;
//!   - comparison modulo normalization;
//!   - URI/IRI-reference resolution;
//!   - static URI/IRI parsing using the `uri`/`iri` macros (provided by
//!     enabling the `macros` feature).
//!   - `serde` support (by enabling the `serde` feature).
//!   - data URL support (by enabling the `data` feature).
//!
//! ## Basic usage
//!
//! You can parse IRI strings by wrapping an `Iri` instance around a `str` slice.
//! Note that no memory allocation occurs using `Iri`, it only borrows the input data.
//! Access to each component is done in constant time.
//!
//! ```rust
//! use iri_rs::Iri;
//!
//! let iri = Iri::parse("https://www.rust-lang.org/foo/bar?query#frag").unwrap();
//!
//! assert_eq!(iri.scheme(), "https");
//! assert_eq!(iri.authority(), Some("www.rust-lang.org"));
//! assert_eq!(iri.path(), "/foo/bar");
//! assert_eq!(iri.query(), Some("query"));
//! assert_eq!(iri.fragment(), Some("frag"));
//! ```
//!
//! IRIs can be created and modified using the `IriBuf` type.
//! With this type, the IRI is held in a single buffer,
//! modified in-place to reduce memory allocation and optimize memory accesses.
//! This also allows the conversion from `IriBuf` into `Iri`.
//!
//! ```rust
//! use iri_rs::IriBuf;
//!
//! let mut iri = IriBuf::new("https://www.rust-lang.org").unwrap();
//! iri.set_authority(Some("www.rust-lang.org:40")).unwrap();
//! iri.set_path("/foo/bar").unwrap();
//! iri.set_query(Some("query")).unwrap();
//! iri.set_fragment(Some("fragment")).unwrap();
//!
//! assert_eq!(iri, "https://www.rust-lang.org:40/foo/bar?query#fragment");
//! ```
//!
//! The `try_into` method is used to ensure that each string is syntactically correct with regard to its corresponding component (for instance, it is not possible to replace `"query"` with `"query?"` since `?` is not a valid query character).
//!
//! ## Detailed Usage
//!
//! ### Path manipulation
//!
//! The IRI path is accessed through the `path` or `path_mut` methods.
//! It is possible to access the segments of a path using the iterator returned by the `segments` method.
//!
//! ```rust
//! use iri_rs::Iri;
//! let iri = Iri::parse("https://www.rust-lang.org/foo/bar?query#frag").unwrap();
//! for segment in iri.path_segments() {
//!     println!("{}", segment);
//! }
//! ```
//!
//! One can use the `normalized_segments` method to iterate over the normalized
//! version of the path where dot segments (`.` and `..`) are removed.
//! In addition, it is possible to push or pop segments to a path using the
//! corresponding methods:
//! ```rust
//! use iri_rs::IriBuf;
//! let mut iri = IriBuf::new("https://rust-lang.org/a/c").unwrap();
//! iri.set_path("/a/b/c/").unwrap();
//! assert_eq!(iri.path(), "/a/b/c/");
//! ```
//!
//! ### IRI references
//!
//! This crate provides the two types `IriRef` and `IriRefBuf` to represent
//! IRI references. An IRI reference is either an IRI or a relative IRI.
//! Contrarily to regular IRIs, relative IRI references may have no scheme.
//!
//! ```rust
//! use iri_rs::{IriRef, IriRefBuf};
//! let mut iri_ref = IriRefBuf::default(); // empty reference.
//! iri_ref.set_scheme(Some("https")).unwrap();
//! iri_ref.set_authority(Some("example.com")).unwrap();
//! assert!(iri_ref.as_iri().is_some());
//! ```
//!
//! Given a base IRI, references can be resolved into a regular IRI using the
//! [Reference Resolution Algorithm](https://tools.ietf.org/html/rfc3986#section-5)
//! defined in [RFC 3986](https://tools.ietf.org/html/rfc3986).
//! This crate provides a *strict* implementation of this algorithm.
//!
//! ```rust
//! use iri_rs::{Iri, IriRefBuf};
//! let base_iri = Iri::parse("http://a/b/c/d;p?q").unwrap();
//! let mut iri_ref = IriRefBuf::new("g;x=1/../y").unwrap();
//!
//! let resolved = iri_ref.resolved(&base_iri).unwrap();
//! assert_eq!(resolved, "http://a/b/c/y");
//!
//! iri_ref.resolve(&base_iri).unwrap();
//! assert_eq!(iri_ref, "http://a/b/c/y");
//! ```
//!
//! This crate implements
//! [Errata 4547](https://www.rfc-editor.org/errata/eid4547) about the
//! abnormal use of dot segments in relative paths.
//! This means that for instance, the path `a/b/../../../` is normalized into
//! `../`.
//!
//! ### IRI comparison
//!
//! Here are the features of the IRI comparison method implemented in this crate.
//!
//! #### Protocol agnostic
//!
//! This implementation does not know anything about existing protocols.
//! For instance, even if the
//! [HTTP protocol](https://en.wikipedia.org/wiki/Hypertext_Transfer_Protocol)
//! defines `80` as the default port,
//! the two IRIs `http://example.org` and `http://example.org:80` are **not** equivalent.
//!
//! #### Every `/` counts
//!
//! The path `/foo/bar` is **not** equivalent to `/foo/bar/`.
//!
//! #### Path normalization
//!
//! Paths are normalized during comparison by removing dot segments (`.` and `..`).
//! This means for instance that the paths `a/b/c` and `a/../a/./b/../b/c` **are**
//! equivalent.
//! Note however that this crate implements
//! [Errata 4547](https://www.rfc-editor.org/errata/eid4547) about the
//! abnormal use of dot segments in relative paths.
//! This means that for instance, the IRI `http:a/b/../../../` is equivalent to
//! `http:../` and **not** `http:`.
//!
//! #### Percent-encoded characters
//!
//! Thanks to the [`pct-str` crate](https://crates.io/crates/pct-str),
//! percent encoded characters are correctly handled.
//! The two IRIs `http://example.org` and `http://exa%6dple.org` **are** equivalent.
pub use iri_rs_core::*;

#[cfg(feature = "static")]
pub use iri_rs_static::*;

#[cfg(feature = "enum")]
pub use iri_rs_enum::*;
