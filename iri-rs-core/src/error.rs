use std::{error::Error, fmt, net::AddrParseError};

#[derive(Debug)]
pub struct IriParseError {
    pub(crate) kind: IriParseErrorKind,
}

impl IriParseError {
    pub fn kind(&self) -> &IriParseErrorKind {
        &self.kind
    }
}

impl fmt::Display for IriParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            IriParseErrorKind::NoScheme => write!(f, "No scheme found in an absolute IRI"),
            IriParseErrorKind::EmptyScheme => write!(f, "Empty schemes are not allowed"),
            IriParseErrorKind::InvalidSchemeCharacter(c) => {
                write!(f, "Invalid character '{c}' in scheme")
            }
            IriParseErrorKind::InvalidHostCharacter(c) => {
                write!(f, "Invalid character '{c}' in host")
            }
            IriParseErrorKind::UnmatchedHostBracket => {
                write!(f, "'[' bracket must be paired with a closing one in host")
            }
            IriParseErrorKind::InvalidHostIp(e) => write!(f, "Invalid host IP ({e})"),
            IriParseErrorKind::InvalidPortCharacter(c) => write!(f, "Invalid character '{c}'"),
            IriParseErrorKind::InvalidIriCodePoint(c) => {
                write!(f, "Invalid IRI code point '{c}'")
            }
            IriParseErrorKind::InvalidPercentEncoding(cs) => {
                write!(
                    f,
                    "Invalid IRI percent encoding '{}'",
                    cs.iter().flatten().collect::<String>()
                )
            }
            IriParseErrorKind::PathStartingWithTwoSlashes => {
                write!(f, "An IRI path is not allowed to start with //")
            }
            IriParseErrorKind::NonAsciiInUri => write!(f, "Non-ASCII character in URI"),
        }
    }
}

impl Error for IriParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let IriParseErrorKind::InvalidHostIp(e) = &self.kind {
            Some(e)
        } else {
            None
        }
    }
}

impl From<IriParseErrorKind> for IriParseError {
    fn from(kind: IriParseErrorKind) -> Self {
        Self { kind }
    }
}

#[derive(Debug)]
pub enum IriParseErrorKind {
    NoScheme,
    EmptyScheme,
    InvalidSchemeCharacter(char),
    UnmatchedHostBracket,
    InvalidHostCharacter(char),
    InvalidHostIp(AddrParseError),
    InvalidPortCharacter(char),
    InvalidIriCodePoint(char),
    InvalidPercentEncoding([Option<char>; 3]),
    PathStartingWithTwoSlashes,
    NonAsciiInUri,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid IRI")]
pub struct InvalidIri<T>(pub T);

#[derive(Debug, thiserror::Error)]
#[error("invalid IRI reference")]
pub struct InvalidIriRef<T>(pub T);

#[derive(Debug, thiserror::Error)]
#[error("invalid URI")]
pub struct InvalidUri<T>(pub T);

#[derive(Debug, thiserror::Error)]
#[error("invalid URI reference")]
pub struct InvalidUriRef<T>(pub T);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_construct() {
        let _ = IriParseError::from(IriParseErrorKind::NoScheme);
        let _ = IriParseError::from(IriParseErrorKind::EmptyScheme);
        let _ = IriParseError::from(IriParseErrorKind::InvalidSchemeCharacter('!'));
        let _ = IriParseError::from(IriParseErrorKind::UnmatchedHostBracket);
        let _ = IriParseError::from(IriParseErrorKind::InvalidHostCharacter(' '));
        let _ = IriParseError::from(IriParseErrorKind::InvalidPortCharacter('a'));
        let _ = IriParseError::from(IriParseErrorKind::InvalidIriCodePoint('\0'));
        let _ = IriParseError::from(IriParseErrorKind::InvalidPercentEncoding([Some('%'), None, None]));
        let _ = IriParseError::from(IriParseErrorKind::PathStartingWithTwoSlashes);
        let _ = IriParseError::from(IriParseErrorKind::NonAsciiInUri);
        let _: InvalidIri<&str> = InvalidIri("x");
        let _: InvalidIriRef<&str> = InvalidIriRef("x");
        let _: InvalidUri<&str> = InvalidUri("x");
        let _: InvalidUriRef<&str> = InvalidUriRef("x");
    }
}
