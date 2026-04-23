//! Mutation methods on owning buffer types (`IriBuf`, `IriRefBuf`, `UriBuf`, `UriRefBuf`).

use crate::{
    error::{InvalidIri, InvalidUri, IriParseError},
    parse::find_iri_ref_positions,
    resolve::resolve,
    types::{Iri, IriBuf, IriRef, IriRefBuf, Uri, UriBuf, UriRef, UriRefBuf},
    validate::{
        validate_authority_body, validate_fragment, validate_iri_ref, validate_path,
        validate_query, validate_resolved_path, validate_scheme,
    },
};

impl IriRefBuf {
    pub fn set_scheme(&mut self, scheme: Option<&str>) -> Result<(), IriParseError> {
        if let Some(s) = scheme {
            validate_scheme(s)?;
        }
        rebuild(self, Edit::Scheme(scheme.map(str::to_string)), true)
    }

    pub fn set_authority(&mut self, authority: Option<&str>) -> Result<(), IriParseError> {
        if let Some(a) = authority {
            validate_authority_body(a, true)?;
        }
        rebuild(self, Edit::Authority(authority.map(str::to_string)), true)
    }

    pub fn set_path(&mut self, path: &str) -> Result<(), IriParseError> {
        validate_path(path, true)?;
        rebuild(self, Edit::Path(path.to_string()), true)
    }

    pub fn set_query(&mut self, query: Option<&str>) -> Result<(), IriParseError> {
        if let Some(q) = query {
            validate_query(q, true)?;
        }
        rebuild(self, Edit::Query(query.map(str::to_string)), true)
    }

    pub fn set_fragment(&mut self, fragment: Option<&str>) -> Result<(), IriParseError> {
        if let Some(f) = fragment {
            validate_fragment(f, true)?;
        }
        rebuild(self, Edit::Fragment(fragment.map(str::to_string)), true)
    }

    pub fn resolve<T: std::ops::Deref<Target = str>>(
        &mut self,
        base: &Iri<T>,
    ) -> Result<(), IriParseError> {
        let mut out = String::with_capacity(self.as_str().len() + base.as_str().len());
        let pos = resolve(
            (base.as_str(), base.positions()),
            (self.as_str(), self.positions),
            &mut out,
        );
        validate_resolved_path(&out, pos)?;
        *self = IriRefBuf::from_raw_parts(out, pos);
        Ok(())
    }

    pub fn try_into_iri(self) -> Result<IriBuf, InvalidIri<IriRefBuf>> {
        if self.is_absolute() {
            let p = self.positions;
            Ok(Iri::from_raw_parts(self.into_inner(), p))
        } else {
            Err(InvalidIri(self))
        }
    }

    pub fn as_iri(&self) -> Option<Iri<&str>> {
        if self.is_absolute() {
            Some(Iri::from_raw_parts(self.as_str(), self.positions))
        } else {
            None
        }
    }
}

impl Default for IriRefBuf {
    fn default() -> Self {
        Self::parse_unchecked(String::new())
    }
}

impl Default for UriRefBuf {
    fn default() -> Self {
        Self::parse_unchecked(String::new())
    }
}

impl IriBuf {
    pub fn set_scheme(&mut self, scheme: &str) -> Result<(), IriParseError> {
        validate_scheme(scheme)?;
        let p = self.positions();
        let iri = std::mem::take(&mut self.0.iri);
        let mut r = IriRefBuf::from_raw_parts(iri, p);
        rebuild(&mut r, Edit::Scheme(Some(scheme.to_string())), true)?;
        *self = Iri::from_raw_parts(r.iri, r.positions);
        Ok(())
    }

    pub fn set_authority(&mut self, authority: Option<&str>) -> Result<(), IriParseError> {
        let p = self.positions();
        let iri = std::mem::take(&mut self.0.iri);
        let mut r = IriRefBuf::from_raw_parts(iri, p);
        r.set_authority(authority)?;
        *self = Iri::from_raw_parts(r.iri, r.positions);
        Ok(())
    }

    pub fn set_path(&mut self, path: &str) -> Result<(), IriParseError> {
        let p = self.positions();
        let iri = std::mem::take(&mut self.0.iri);
        let mut r = IriRefBuf::from_raw_parts(iri, p);
        r.set_path(path)?;
        *self = Iri::from_raw_parts(r.iri, r.positions);
        Ok(())
    }

    pub fn set_query(&mut self, query: Option<&str>) -> Result<(), IriParseError> {
        let p = self.positions();
        let iri = std::mem::take(&mut self.0.iri);
        let mut r = IriRefBuf::from_raw_parts(iri, p);
        r.set_query(query)?;
        *self = Iri::from_raw_parts(r.iri, r.positions);
        Ok(())
    }

    pub fn set_fragment(&mut self, fragment: Option<&str>) -> Result<(), IriParseError> {
        let p = self.positions();
        let iri = std::mem::take(&mut self.0.iri);
        let mut r = IriRefBuf::from_raw_parts(iri, p);
        r.set_fragment(fragment)?;
        *self = Iri::from_raw_parts(r.iri, r.positions);
        Ok(())
    }
}

impl UriRefBuf {
    pub fn set_scheme(&mut self, scheme: Option<&str>) -> Result<(), IriParseError> {
        if let Some(s) = scheme {
            validate_scheme(s)?;
        }
        rebuild_uri(self, Edit::Scheme(scheme.map(str::to_string)))
    }
    pub fn set_authority(&mut self, authority: Option<&str>) -> Result<(), IriParseError> {
        if let Some(a) = authority {
            validate_authority_body(a, false)?;
        }
        rebuild_uri(self, Edit::Authority(authority.map(str::to_string)))
    }
    pub fn set_path(&mut self, path: &str) -> Result<(), IriParseError> {
        validate_path(path, false)?;
        rebuild_uri(self, Edit::Path(path.to_string()))
    }
    pub fn set_query(&mut self, query: Option<&str>) -> Result<(), IriParseError> {
        if let Some(q) = query {
            validate_query(q, false)?;
        }
        rebuild_uri(self, Edit::Query(query.map(str::to_string)))
    }
    pub fn set_fragment(&mut self, fragment: Option<&str>) -> Result<(), IriParseError> {
        if let Some(f) = fragment {
            validate_fragment(f, false)?;
        }
        rebuild_uri(self, Edit::Fragment(fragment.map(str::to_string)))
    }
    pub fn resolve<T: std::ops::Deref<Target = str>>(
        &mut self,
        base: &Uri<T>,
    ) -> Result<(), IriParseError> {
        let mut out = String::with_capacity(self.as_str().len() + base.as_str().len());
        let pos = resolve(
            (base.as_str(), base.positions()),
            (self.as_str(), self.positions),
            &mut out,
        );
        validate_resolved_path(&out, pos)?;
        *self = UriRefBuf::from_raw_parts(out, pos);
        Ok(())
    }

    pub fn try_into_uri(self) -> Result<UriBuf, InvalidUri<UriRefBuf>> {
        if self.is_absolute() {
            let p = self.positions;
            Ok(Uri::from_raw_parts(self.into_inner(), p))
        } else {
            Err(InvalidUri(self))
        }
    }

    pub fn as_uri(&self) -> Option<Uri<&str>> {
        if self.is_absolute() {
            Some(Uri::from_raw_parts(self.as_str(), self.positions))
        } else {
            None
        }
    }
}

impl UriBuf {
    pub fn set_scheme(&mut self, scheme: &str) -> Result<(), IriParseError> {
        let p = self.positions();
        let uri = std::mem::take(&mut self.0.uri);
        let mut r = UriRefBuf::from_raw_parts(uri, p);
        r.set_scheme(Some(scheme))?;
        *self = Uri::from_raw_parts(r.uri, r.positions);
        Ok(())
    }
    pub fn set_authority(&mut self, authority: Option<&str>) -> Result<(), IriParseError> {
        let p = self.positions();
        let uri = std::mem::take(&mut self.0.uri);
        let mut r = UriRefBuf::from_raw_parts(uri, p);
        r.set_authority(authority)?;
        *self = Uri::from_raw_parts(r.uri, r.positions);
        Ok(())
    }
    pub fn set_path(&mut self, path: &str) -> Result<(), IriParseError> {
        let p = self.positions();
        let uri = std::mem::take(&mut self.0.uri);
        let mut r = UriRefBuf::from_raw_parts(uri, p);
        r.set_path(path)?;
        *self = Uri::from_raw_parts(r.uri, r.positions);
        Ok(())
    }
    pub fn set_query(&mut self, query: Option<&str>) -> Result<(), IriParseError> {
        let p = self.positions();
        let uri = std::mem::take(&mut self.0.uri);
        let mut r = UriRefBuf::from_raw_parts(uri, p);
        r.set_query(query)?;
        *self = Uri::from_raw_parts(r.uri, r.positions);
        Ok(())
    }
    pub fn set_fragment(&mut self, fragment: Option<&str>) -> Result<(), IriParseError> {
        let p = self.positions();
        let uri = std::mem::take(&mut self.0.uri);
        let mut r = UriRefBuf::from_raw_parts(uri, p);
        r.set_fragment(fragment)?;
        *self = Uri::from_raw_parts(r.uri, r.positions);
        Ok(())
    }
}

enum Edit {
    Scheme(Option<String>),
    Authority(Option<String>),
    Path(String),
    Query(Option<String>),
    Fragment(Option<String>),
}

fn rebuild(buf: &mut IriRefBuf, edit: Edit, is_iri: bool) -> Result<(), IriParseError> {
    let out = {
        let s = buf.as_str();
        let (scheme, authority, path, query, fragment) = destructure(s, buf.positions);
        let mut out = String::with_capacity(s.len() + 8);
        match &edit {
            Edit::Scheme(new) => {
                write_iri(&mut out, new.as_deref(), authority, path, query, fragment)
            }
            Edit::Authority(new) => {
                write_iri(&mut out, scheme, new.as_deref(), path, query, fragment)
            }
            Edit::Path(new) => write_iri(&mut out, scheme, authority, new, query, fragment),
            Edit::Query(new) => {
                write_iri(&mut out, scheme, authority, path, new.as_deref(), fragment)
            }
            Edit::Fragment(new) => {
                write_iri(&mut out, scheme, authority, path, query, new.as_deref())
            }
        }
        out
    };
    let positions = find_iri_ref_positions(&out);
    validate_iri_ref(&out, positions, is_iri)?;
    *buf = IriRef::from_raw_parts(out, positions);
    Ok(())
}

fn write_iri(
    out: &mut String,
    scheme: Option<&str>,
    authority: Option<&str>,
    path: &str,
    query: Option<&str>,
    fragment: Option<&str>,
) {
    if let Some(s) = scheme {
        out.push_str(s);
        out.push(':');
    }
    if let Some(a) = authority {
        out.push_str("//");
        out.push_str(a);
    }
    out.push_str(path);
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
}

fn rebuild_uri(buf: &mut UriRefBuf, edit: Edit) -> Result<(), IriParseError> {
    let mut tmp: IriRefBuf = IriRef::from_raw_parts(buf.as_str().to_string(), buf.positions);
    rebuild(&mut tmp, edit, false)?;
    let p = tmp.positions;
    *buf = UriRef::from_raw_parts(tmp.iri, p);
    Ok(())
}

fn destructure(
    s: &str,
    p: crate::parse::Positions,
) -> (Option<&str>, Option<&str>, &str, Option<&str>, Option<&str>) {
    let scheme = if p.scheme_end > 0 {
        Some(&s[..p.scheme_end - 1])
    } else {
        None
    };
    let authority = if p.authority_end > p.scheme_end + 2
        || (p.authority_end == p.scheme_end + 2
            && p.scheme_end + 2 <= s.len()
            && &s[p.scheme_end..p.scheme_end + 2] == "//")
    {
        Some(&s[p.scheme_end + 2..p.authority_end])
    } else {
        None
    };
    let path = &s[p.authority_end..p.path_end];
    let query = if p.query_end > p.path_end {
        Some(&s[p.path_end + 1..p.query_end])
    } else {
        None
    };
    let fragment = if s.len() > p.query_end {
        Some(&s[p.query_end + 1..])
    } else {
        None
    };
    (scheme, authority, path, query, fragment)
}

