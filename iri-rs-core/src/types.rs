//! Generic `Iri<T>`, `IriRef<T>`, `Uri<T>`, `UriRef<T>` — parse-once, cached positions.

use std::{
    borrow::{Borrow, Cow},
    cmp::Ordering,
    convert::TryFrom,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    str::FromStr,
};

use crate::{
    error::{InvalidIri, InvalidUri, InvalidUriRef, IriParseError},
    parse::{Positions, find_iri_positions, find_iri_ref_positions},
    resolve::resolve,
    validate::{validate_iri, validate_iri_ref, validate_resolved_path},
};

// ============================================================================
// IriRef<T>
// ============================================================================

#[derive(Clone, Copy)]
pub struct IriRef<T> {
    pub(crate) iri: T,
    pub(crate) positions: Positions,
}

impl<T: Deref<Target = str>> IriRef<T> {
    pub fn parse(iri: T) -> Result<Self, InvalidIri<T>> {
        let positions = find_iri_ref_positions(&iri);
        match validate_iri_ref(&iri, positions, true) {
            Ok(()) => Ok(Self { iri, positions }),
            Err(_) => Err(InvalidIri(iri)),
        }
    }

    pub fn parse_unchecked(iri: T) -> Self {
        let positions = find_iri_ref_positions(&iri);
        Self { iri, positions }
    }

    #[doc(hidden)]
    pub const fn from_raw_parts(iri: T, positions: Positions) -> Self {
        Self { iri, positions }
    }

    pub fn as_ref(&self) -> IriRef<&str> {
        IriRef { iri: &self.iri, positions: self.positions }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.iri
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.iri
    }

    #[inline]
    pub fn positions(&self) -> Positions {
        self.positions
    }

    #[inline]
    pub fn is_absolute(&self) -> bool {
        self.positions.scheme_end != 0
    }

    #[inline]
    pub fn scheme(&self) -> Option<&str> {
        if self.positions.scheme_end == 0 {
            None
        } else {
            Some(&self.iri[..self.positions.scheme_end - 1])
        }
    }

    #[inline]
    pub fn authority(&self) -> Option<&str> {
        if self.positions.scheme_end + 2 > self.positions.authority_end {
            None
        } else {
            Some(&self.iri[self.positions.scheme_end + 2..self.positions.authority_end])
        }
    }

    #[inline]
    pub fn path(&self) -> &str {
        &self.iri[self.positions.authority_end..self.positions.path_end]
    }

    #[inline]
    pub fn query(&self) -> Option<&str> {
        if self.positions.path_end >= self.positions.query_end {
            None
        } else {
            Some(&self.iri[self.positions.path_end + 1..self.positions.query_end])
        }
    }

    #[inline]
    pub fn fragment(&self) -> Option<&str> {
        if self.positions.query_end >= self.iri.len() {
            None
        } else {
            Some(&self.iri[self.positions.query_end + 1..])
        }
    }

    pub fn resolve_into<U: Deref<Target = str>>(
        &self,
        base: &Iri<U>,
        output: &mut String,
    ) -> Result<(), IriParseError> {
        let positions = resolve(
            (&base.0.iri, base.0.positions),
            (&self.iri, self.positions),
            output,
        );
        validate_resolved_path(output, positions)
    }

    pub fn resolved<U: Deref<Target = str>>(
        &self,
        base: &Iri<U>,
    ) -> Result<IriRef<String>, IriParseError> {
        let mut out = String::with_capacity(base.0.iri.len() + self.iri.len());
        let positions = resolve(
            (&base.0.iri, base.0.positions),
            (&self.iri, self.positions),
            &mut out,
        );
        validate_resolved_path(&out, positions)?;
        Ok(IriRef { iri: out, positions })
    }
}

impl<T: Deref<Target = str>> Deref for IriRef<T> {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        &self.iri
    }
}

impl<T: Deref<Target = str>> AsRef<str> for IriRef<T> {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.iri
    }
}

impl<T: Deref<Target = str>> Borrow<str> for IriRef<T> {
    #[inline]
    fn borrow(&self) -> &str {
        &self.iri
    }
}

impl<T: Deref<Target = str>> fmt::Debug for IriRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.iri, f)
    }
}

impl<T: Deref<Target = str>> fmt::Display for IriRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.iri)
    }
}

impl FromStr for IriRef<String> {
    type Err = InvalidIri<String>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s.to_owned())
    }
}

#[cfg(feature = "fast-hash")]
mod fast_hash_impls {
    use super::*;
    impl<T: Deref<Target = str>> PartialEq for IriRef<T> {
        fn eq(&self, other: &Self) -> bool {
            *self.iri == *other.iri
        }
    }
    impl<T: Deref<Target = str>> Eq for IriRef<T> {}
    impl<T: Deref<Target = str>> Hash for IriRef<T> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            (*self.iri).hash(state)
        }
    }
    impl<T: Deref<Target = str>> PartialOrd for IriRef<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<T: Deref<Target = str>> Ord for IriRef<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            (*self.iri).cmp(&*other.iri)
        }
    }
    impl<T: Deref<Target = str>> PartialEq for Iri<T> {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl<T: Deref<Target = str>> Eq for Iri<T> {}
    impl<T: Deref<Target = str>> Hash for Iri<T> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.0.hash(state)
        }
    }
    impl<T: Deref<Target = str>> PartialOrd for Iri<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<T: Deref<Target = str>> Ord for Iri<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            self.0.cmp(&other.0)
        }
    }
}

#[cfg(not(feature = "fast-hash"))]
mod normalized_impls {
    use super::*;
    use crate::normalize::{iri_cmp, iri_eq, normalized_hash};

    impl<T: Deref<Target = str>> PartialEq for IriRef<T> {
        fn eq(&self, other: &Self) -> bool {
            iri_eq(&self.iri, self.positions, &other.iri, other.positions)
        }
    }
    impl<T: Deref<Target = str>> Eq for IriRef<T> {}
    impl<T: Deref<Target = str>> Hash for IriRef<T> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            normalized_hash(&self.iri, self.positions, state)
        }
    }
    impl<T: Deref<Target = str>> PartialOrd for IriRef<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<T: Deref<Target = str>> Ord for IriRef<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            iri_cmp(&self.iri, self.positions, &other.iri, other.positions)
        }
    }
    impl<T: Deref<Target = str>> PartialEq for Iri<T> {
        fn eq(&self, other: &Self) -> bool {
            iri_eq(
                &self.0.iri,
                self.0.positions,
                &other.0.iri,
                other.0.positions,
            )
        }
    }
    impl<T: Deref<Target = str>> Eq for Iri<T> {}
    impl<T: Deref<Target = str>> Hash for Iri<T> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            normalized_hash(&self.0.iri, self.0.positions, state)
        }
    }
    impl<T: Deref<Target = str>> PartialOrd for Iri<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<T: Deref<Target = str>> Ord for Iri<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            iri_cmp(
                &self.0.iri,
                self.0.positions,
                &other.0.iri,
                other.0.positions,
            )
        }
    }
}

impl<'a> From<IriRef<&'a str>> for IriRef<String> {
    fn from(v: IriRef<&'a str>) -> Self {
        Self { iri: v.iri.into(), positions: v.positions }
    }
}
impl<'a> From<&'a IriRef<String>> for IriRef<&'a str> {
    fn from(v: &'a IriRef<String>) -> Self {
        IriRef { iri: &v.iri, positions: v.positions }
    }
}
impl<'a> From<IriRef<&'a str>> for IriRef<Cow<'a, str>> {
    fn from(v: IriRef<&'a str>) -> Self {
        Self { iri: Cow::Borrowed(v.iri), positions: v.positions }
    }
}
impl From<IriRef<String>> for IriRef<Cow<'_, str>> {
    fn from(v: IriRef<String>) -> Self {
        Self { iri: Cow::Owned(v.iri), positions: v.positions }
    }
}

impl<T: Deref<Target = str>> PartialEq<str> for IriRef<T> {
    fn eq(&self, other: &str) -> bool {
        &*self.iri == other
    }
}
impl<T: Deref<Target = str>> PartialEq<&str> for IriRef<T> {
    fn eq(&self, other: &&str) -> bool {
        &*self.iri == *other
    }
}
impl<T: Deref<Target = str>> PartialEq<String> for IriRef<T> {
    fn eq(&self, other: &String) -> bool {
        &*self.iri == other.as_str()
    }
}

// ============================================================================
// Iri<T>
// ============================================================================

#[derive(Clone, Copy)]
pub struct Iri<T>(pub(crate) IriRef<T>);

impl<T: Deref<Target = str>> Iri<T> {
    pub fn parse(iri: T) -> Result<Self, InvalidIri<T>> {
        let positions = find_iri_positions(&iri);
        match validate_iri(&iri, positions, true) {
            Ok(()) => Ok(Self(IriRef { iri, positions })),
            Err(_) => Err(InvalidIri(iri)),
        }
    }

    pub fn parse_unchecked(iri: T) -> Self {
        let positions = find_iri_positions(&iri);
        Self(IriRef { iri, positions })
    }

    #[doc(hidden)]
    pub const fn from_raw_parts(iri: T, positions: Positions) -> Self {
        Self(IriRef { iri, positions })
    }

    pub fn as_ref(&self) -> Iri<&str> {
        Iri(self.0.as_ref())
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
    pub fn positions(&self) -> Positions {
        self.0.positions
    }

    #[inline]
    pub fn scheme(&self) -> &str {
        self.0.scheme().expect("absolute IRI has scheme")
    }

    #[inline]
    pub fn authority(&self) -> Option<&str> {
        self.0.authority()
    }
    #[inline]
    pub fn path(&self) -> &str {
        self.0.path()
    }
    #[inline]
    pub fn query(&self) -> Option<&str> {
        self.0.query()
    }
    #[inline]
    pub fn fragment(&self) -> Option<&str> {
        self.0.fragment()
    }

    pub fn as_iri_ref(&self) -> &IriRef<T> {
        &self.0
    }
}

impl<T: Deref<Target = str>> Deref for Iri<T> {
    type Target = str;
    fn deref(&self) -> &str {
        self.0.deref()
    }
}

impl<T: Deref<Target = str>> AsRef<str> for Iri<T> {
    fn as_ref(&self) -> &str {
        <IriRef<T> as AsRef<str>>::as_ref(&self.0)
    }
}
impl<T: Deref<Target = str>> Borrow<str> for Iri<T> {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}

impl<T: Deref<Target = str>> fmt::Debug for Iri<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl<T: Deref<Target = str>> fmt::Display for Iri<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for Iri<String> {
    type Err = InvalidIri<String>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s.to_owned())
    }
}

impl<T: Deref<Target = str>> From<Iri<T>> for IriRef<T> {
    fn from(v: Iri<T>) -> Self {
        v.0
    }
}

impl<T: Deref<Target = str>> TryFrom<IriRef<T>> for Iri<T> {
    type Error = InvalidIri<T>;
    fn try_from(v: IriRef<T>) -> Result<Self, Self::Error> {
        if v.is_absolute() {
            Ok(Self(v))
        } else {
            Err(InvalidIri(v.iri))
        }
    }
}

impl<'a> From<Iri<&'a str>> for Iri<String> {
    fn from(v: Iri<&'a str>) -> Self {
        Self(v.0.into())
    }
}
impl<'a> From<&'a Iri<String>> for Iri<&'a str> {
    fn from(v: &'a Iri<String>) -> Self {
        Iri(IriRef { iri: &v.0.iri, positions: v.0.positions })
    }
}

impl<T: Deref<Target = str>> PartialEq<str> for Iri<T> {
    fn eq(&self, other: &str) -> bool {
        self.0.eq(other)
    }
}
impl<T: Deref<Target = str>> PartialEq<&str> for Iri<T> {
    fn eq(&self, other: &&str) -> bool {
        self.0.eq(other)
    }
}
impl<T: Deref<Target = str>> PartialEq<String> for Iri<T> {
    fn eq(&self, other: &String) -> bool {
        self.0.eq(other)
    }
}

// ============================================================================
// UriRef<T> / Uri<T>
// ============================================================================

#[derive(Clone, Copy)]
pub struct UriRef<T> {
    pub(crate) uri: T,
    pub(crate) positions: Positions,
}

impl<T: Deref<Target = str>> UriRef<T> {
    pub fn parse(uri: T) -> Result<Self, InvalidUriRef<T>> {
        if !uri.is_ascii() {
            return Err(InvalidUriRef(uri));
        }
        let positions = find_iri_ref_positions(&uri);
        match validate_iri_ref(&uri, positions, false) {
            Ok(()) => Ok(Self { uri, positions }),
            Err(_) => Err(InvalidUriRef(uri)),
        }
    }

    pub fn parse_unchecked(uri: T) -> Self {
        let positions = find_iri_ref_positions(&uri);
        Self { uri, positions }
    }

    #[doc(hidden)]
    pub const fn from_raw_parts(uri: T, positions: Positions) -> Self {
        Self { uri, positions }
    }

    pub fn as_ref(&self) -> UriRef<&str> {
        UriRef { uri: &self.uri, positions: self.positions }
    }
    pub fn as_str(&self) -> &str {
        &self.uri
    }
    pub fn into_inner(self) -> T {
        self.uri
    }
    pub fn positions(&self) -> Positions {
        self.positions
    }
    pub fn is_absolute(&self) -> bool {
        self.positions.scheme_end != 0
    }

    #[inline]
    pub fn scheme(&self) -> Option<&str> {
        if self.positions.scheme_end == 0 {
            None
        } else {
            Some(&self.uri[..self.positions.scheme_end - 1])
        }
    }
    #[inline]
    pub fn authority(&self) -> Option<&str> {
        if self.positions.scheme_end + 2 > self.positions.authority_end {
            None
        } else {
            Some(&self.uri[self.positions.scheme_end + 2..self.positions.authority_end])
        }
    }
    #[inline]
    pub fn path(&self) -> &str {
        &self.uri[self.positions.authority_end..self.positions.path_end]
    }
    #[inline]
    pub fn query(&self) -> Option<&str> {
        if self.positions.path_end >= self.positions.query_end {
            None
        } else {
            Some(&self.uri[self.positions.path_end + 1..self.positions.query_end])
        }
    }
    #[inline]
    pub fn fragment(&self) -> Option<&str> {
        if self.positions.query_end >= self.uri.len() {
            None
        } else {
            Some(&self.uri[self.positions.query_end + 1..])
        }
    }

    pub fn resolve_into<U: Deref<Target = str>>(
        &self,
        base: &Uri<U>,
        output: &mut String,
    ) -> Result<(), IriParseError> {
        let positions = resolve(
            (&base.0.uri, base.0.positions),
            (&self.uri, self.positions),
            output,
        );
        validate_resolved_path(output, positions)
    }

    pub fn resolved<U: Deref<Target = str>>(
        &self,
        base: &Uri<U>,
    ) -> Result<UriRef<String>, IriParseError> {
        let mut out = String::with_capacity(base.0.uri.len() + self.uri.len());
        let positions = resolve(
            (&base.0.uri, base.0.positions),
            (&self.uri, self.positions),
            &mut out,
        );
        validate_resolved_path(&out, positions)?;
        Ok(UriRef { uri: out, positions })
    }
}

impl<T: Deref<Target = str>> Deref for UriRef<T> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.uri
    }
}
impl<T: Deref<Target = str>> AsRef<str> for UriRef<T> {
    fn as_ref(&self) -> &str {
        &self.uri
    }
}
impl<T: Deref<Target = str>> Borrow<str> for UriRef<T> {
    fn borrow(&self) -> &str {
        &self.uri
    }
}
impl<T: Deref<Target = str>> fmt::Debug for UriRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.uri, f)
    }
}
impl<T: Deref<Target = str>> fmt::Display for UriRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.uri)
    }
}

impl FromStr for UriRef<String> {
    type Err = InvalidUriRef<String>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s.to_owned())
    }
}

impl<'a> From<UriRef<&'a str>> for UriRef<String> {
    fn from(v: UriRef<&'a str>) -> Self {
        UriRef { uri: v.uri.into(), positions: v.positions }
    }
}
impl<'a> From<&'a UriRef<String>> for UriRef<&'a str> {
    fn from(v: &'a UriRef<String>) -> Self {
        UriRef { uri: &v.uri, positions: v.positions }
    }
}

#[derive(Clone, Copy)]
pub struct Uri<T>(pub(crate) UriRef<T>);

impl<T: Deref<Target = str>> Uri<T> {
    pub fn parse(uri: T) -> Result<Self, InvalidUri<T>> {
        if !uri.is_ascii() {
            return Err(InvalidUri(uri));
        }
        let positions = find_iri_positions(&uri);
        match validate_iri(&uri, positions, false) {
            Ok(()) => Ok(Self(UriRef { uri, positions })),
            Err(_) => Err(InvalidUri(uri)),
        }
    }
    pub fn parse_unchecked(uri: T) -> Self {
        let positions = find_iri_positions(&uri);
        Self(UriRef { uri, positions })
    }

    #[doc(hidden)]
    pub const fn from_raw_parts(uri: T, positions: Positions) -> Self {
        Self(UriRef { uri, positions })
    }

    pub fn as_ref(&self) -> Uri<&str> {
        Uri(self.0.as_ref())
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
    pub fn positions(&self) -> Positions {
        self.0.positions
    }
    #[inline]
    pub fn scheme(&self) -> &str {
        self.0.scheme().expect("absolute URI has scheme")
    }
    #[inline]
    pub fn authority(&self) -> Option<&str> {
        self.0.authority()
    }
    #[inline]
    pub fn path(&self) -> &str {
        self.0.path()
    }
    #[inline]
    pub fn query(&self) -> Option<&str> {
        self.0.query()
    }
    #[inline]
    pub fn fragment(&self) -> Option<&str> {
        self.0.fragment()
    }
    pub fn as_uri_ref(&self) -> &UriRef<T> {
        &self.0
    }
}

impl<T: Deref<Target = str>> Deref for Uri<T> {
    type Target = str;
    fn deref(&self) -> &str {
        self.0.deref()
    }
}
impl<T: Deref<Target = str>> AsRef<str> for Uri<T> {
    fn as_ref(&self) -> &str {
        <UriRef<T> as AsRef<str>>::as_ref(&self.0)
    }
}
impl<T: Deref<Target = str>> Borrow<str> for Uri<T> {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}
impl<T: Deref<Target = str>> fmt::Debug for Uri<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl<T: Deref<Target = str>> fmt::Display for Uri<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for Uri<String> {
    type Err = InvalidUri<String>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s.to_owned())
    }
}

impl<T: Deref<Target = str>> From<Uri<T>> for UriRef<T> {
    fn from(v: Uri<T>) -> Self {
        v.0
    }
}

impl<T: Deref<Target = str>> TryFrom<UriRef<T>> for Uri<T> {
    type Error = InvalidUri<T>;
    fn try_from(v: UriRef<T>) -> Result<Self, Self::Error> {
        if v.is_absolute() {
            Ok(Self(v))
        } else {
            Err(InvalidUri(v.uri))
        }
    }
}

impl<'a> From<Uri<&'a str>> for Uri<String> {
    fn from(v: Uri<&'a str>) -> Self {
        Self(v.0.into())
    }
}

#[cfg(feature = "fast-hash")]
mod uri_fast {
    use super::*;
    impl<T: Deref<Target = str>> PartialEq for UriRef<T> {
        fn eq(&self, other: &Self) -> bool {
            *self.uri == *other.uri
        }
    }
    impl<T: Deref<Target = str>> Eq for UriRef<T> {}
    impl<T: Deref<Target = str>> Hash for UriRef<T> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            (*self.uri).hash(state)
        }
    }
    impl<T: Deref<Target = str>> PartialOrd for UriRef<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<T: Deref<Target = str>> Ord for UriRef<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            (*self.uri).cmp(&*other.uri)
        }
    }
    impl<T: Deref<Target = str>> PartialEq for Uri<T> {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl<T: Deref<Target = str>> Eq for Uri<T> {}
    impl<T: Deref<Target = str>> Hash for Uri<T> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.0.hash(state)
        }
    }
    impl<T: Deref<Target = str>> PartialOrd for Uri<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<T: Deref<Target = str>> Ord for Uri<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            self.0.cmp(&other.0)
        }
    }
}

#[cfg(not(feature = "fast-hash"))]
mod uri_norm {
    use super::*;
    use crate::normalize::{iri_cmp, iri_eq, normalized_hash};
    impl<T: Deref<Target = str>> PartialEq for UriRef<T> {
        fn eq(&self, other: &Self) -> bool {
            iri_eq(&self.uri, self.positions, &other.uri, other.positions)
        }
    }
    impl<T: Deref<Target = str>> Eq for UriRef<T> {}
    impl<T: Deref<Target = str>> Hash for UriRef<T> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            normalized_hash(&self.uri, self.positions, state)
        }
    }
    impl<T: Deref<Target = str>> PartialOrd for UriRef<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<T: Deref<Target = str>> Ord for UriRef<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            iri_cmp(&self.uri, self.positions, &other.uri, other.positions)
        }
    }
    impl<T: Deref<Target = str>> PartialEq for Uri<T> {
        fn eq(&self, other: &Self) -> bool {
            iri_eq(
                &self.0.uri,
                self.0.positions,
                &other.0.uri,
                other.0.positions,
            )
        }
    }
    impl<T: Deref<Target = str>> Eq for Uri<T> {}
    impl<T: Deref<Target = str>> Hash for Uri<T> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            normalized_hash(&self.0.uri, self.0.positions, state)
        }
    }
    impl<T: Deref<Target = str>> PartialOrd for Uri<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<T: Deref<Target = str>> Ord for Uri<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            iri_cmp(
                &self.0.uri,
                self.0.positions,
                &other.0.uri,
                other.0.positions,
            )
        }
    }
}

impl<T: Deref<Target = str>> PartialEq<str> for UriRef<T> {
    fn eq(&self, other: &str) -> bool {
        &*self.uri == other
    }
}
impl<T: Deref<Target = str>> PartialEq<&str> for UriRef<T> {
    fn eq(&self, other: &&str) -> bool {
        &*self.uri == *other
    }
}
impl<T: Deref<Target = str>> PartialEq<String> for UriRef<T> {
    fn eq(&self, other: &String) -> bool {
        &*self.uri == other.as_str()
    }
}
impl<T: Deref<Target = str>> PartialEq<str> for Uri<T> {
    fn eq(&self, other: &str) -> bool {
        self.0.eq(other)
    }
}
impl<T: Deref<Target = str>> PartialEq<&str> for Uri<T> {
    fn eq(&self, other: &&str) -> bool {
        self.0.eq(other)
    }
}
impl<T: Deref<Target = str>> PartialEq<String> for Uri<T> {
    fn eq(&self, other: &String) -> bool {
        self.0.eq(other)
    }
}

pub type IriBuf = Iri<String>;
pub type IriRefBuf = IriRef<String>;
pub type UriBuf = Uri<String>;
pub type UriRefBuf = UriRef<String>;

impl Iri<String> {
    pub fn new(s: impl Into<String>) -> Result<Self, InvalidIri<String>> {
        Self::parse(s.into())
    }
    pub fn from_vec(buffer: Vec<u8>) -> Result<Self, InvalidIri<Vec<u8>>> {
        match simdutf8::basic::from_utf8(&buffer) {
            Ok(_) => {
                let s = unsafe { String::from_utf8_unchecked(buffer) };
                Self::new(s).map_err(|InvalidIri(s)| InvalidIri(s.into_bytes()))
            }
            Err(_) => Err(InvalidIri(buffer)),
        }
    }
}
impl IriRef<String> {
    pub fn new(s: impl Into<String>) -> Result<Self, InvalidIri<String>> {
        Self::parse(s.into())
    }
    pub fn from_vec(buffer: Vec<u8>) -> Result<Self, InvalidIri<Vec<u8>>> {
        match simdutf8::basic::from_utf8(&buffer) {
            Ok(_) => {
                let s = unsafe { String::from_utf8_unchecked(buffer) };
                Self::new(s).map_err(|InvalidIri(s)| InvalidIri(s.into_bytes()))
            }
            Err(_) => Err(InvalidIri(buffer)),
        }
    }
}
impl Uri<String> {
    pub fn new(s: impl Into<Vec<u8>>) -> Result<Self, InvalidUri<Vec<u8>>> {
        let bytes = s.into();
        if !bytes.is_ascii() {
            return Err(InvalidUri(bytes));
        }
        let s = unsafe { String::from_utf8_unchecked(bytes) };
        Self::parse(s).map_err(|InvalidUri(s)| InvalidUri(s.into_bytes()))
    }
}
impl UriRef<String> {
    pub fn new(s: impl Into<Vec<u8>>) -> Result<Self, InvalidUriRef<Vec<u8>>> {
        let bytes = s.into();
        if !bytes.is_ascii() {
            return Err(InvalidUriRef(bytes));
        }
        let s = unsafe { String::from_utf8_unchecked(bytes) };
        Self::parse(s).map_err(|InvalidUriRef(s)| InvalidUriRef(s.into_bytes()))
    }
}

#[cfg(feature = "serde")]
mod serde_impls {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    impl<T: Deref<Target = str> + Serialize> Serialize for IriRef<T> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            self.iri.serialize(s)
        }
    }
    impl<'de, T: Deref<Target = str> + Deserialize<'de>> Deserialize<'de> for IriRef<T> {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            use serde::de::Error;
            Self::parse(T::deserialize(d)?)
                .map_err(|e| Error::custom(format!("{:?}", e.0.deref())))
        }
    }
    impl<T: Deref<Target = str> + Serialize> Serialize for Iri<T> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            self.0.serialize(s)
        }
    }
    impl<'de, T: Deref<Target = str> + Deserialize<'de>> Deserialize<'de> for Iri<T> {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            use serde::de::Error;
            let r = IriRef::<T>::deserialize(d)?;
            Self::try_from(r).map_err(|e| Error::custom(format!("{:?}", e.0.deref())))
        }
    }
    impl<T: Deref<Target = str> + Serialize> Serialize for UriRef<T> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            self.uri.serialize(s)
        }
    }
    impl<'de, T: Deref<Target = str> + Deserialize<'de>> Deserialize<'de> for UriRef<T> {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            use serde::de::Error;
            Self::parse(T::deserialize(d)?)
                .map_err(|e| Error::custom(format!("{:?}", e.0.deref())))
        }
    }
    impl<T: Deref<Target = str> + Serialize> Serialize for Uri<T> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            self.0.serialize(s)
        }
    }
    impl<'de, T: Deref<Target = str> + Deserialize<'de>> Deserialize<'de> for Uri<T> {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            use serde::de::Error;
            let r = UriRef::<T>::deserialize(d)?;
            Self::try_from(r).map_err(|e| Error::custom(format!("{:?}", e.0.deref())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_iri() {
        let iri = Iri::parse("http://a/b").unwrap();
        assert_eq!(iri.scheme(), "http");
        assert_eq!(iri.authority(), Some("a"));
        assert_eq!(iri.path(), "/b");
    }

    #[test]
    fn iri_buf_new() {
        let _: IriBuf = IriBuf::new("http://a/b").unwrap();
    }

    #[test]
    fn relative_reject_by_iri() {
        assert!(Iri::parse("/rel").is_err());
    }

    #[test]
    fn iri_ref_accepts_relative() {
        assert!(IriRef::parse("/rel").is_ok());
    }
}
