//! IRI / URI parsing, validation, resolution, and RFC 3986/3987 §6.2.2 equivalence.

pub mod components;
pub mod error;
pub mod mutate;
pub mod normalize;
pub mod parse;
pub mod relativize;
pub mod resolve;
pub mod types;
pub mod validate;

pub use error::{InvalidIri, InvalidIriRef, InvalidUri, InvalidUriRef, IriParseError, IriParseErrorKind};
pub use parse::Positions;
pub use types::{Iri, IriBuf, IriRef, IriRefBuf, Uri, UriBuf, UriRef, UriRefBuf};

/// Compatibility namespace.
pub mod iri {
    pub use crate::{
        error::{InvalidIri, InvalidIriRef},
        types::{Iri, IriBuf, IriRef, IriRefBuf},
    };
}

pub mod uri {
    pub use crate::{
        error::{InvalidUri, InvalidUriRef},
        types::{Uri, UriBuf, UriRef, UriRefBuf},
    };
}
