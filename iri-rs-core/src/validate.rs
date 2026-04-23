//! Hand-rolled grammar validators — oxiri port with `is_iri` flag for URI vs IRI flavors.
use std::{net::Ipv6Addr, str::FromStr};

use crate::{
    error::{IriParseError, IriParseErrorKind},
    parse::Positions,
};

// --- char-class tables -------------------------------------------------------

pub(crate) const fn set_alpha(a: &mut [bool; 256]) {
    let mut i = b'a';
    while i <= b'z' {
        a[i as usize] = true;
        i += 1;
    }
    let mut i = b'A';
    while i <= b'Z' {
        a[i as usize] = true;
        i += 1;
    }
}
pub(crate) const fn set_digit(a: &mut [bool; 256]) {
    let mut i = b'0';
    while i <= b'9' {
        a[i as usize] = true;
        i += 1;
    }
}
pub(crate) const fn set_unreserved(a: &mut [bool; 256]) {
    set_alpha(a);
    set_digit(a);
    a[b'-' as usize] = true;
    a[b'.' as usize] = true;
    a[b'_' as usize] = true;
    a[b'~' as usize] = true;
}
pub(crate) const fn set_sub_delims(a: &mut [bool; 256]) {
    a[b'!' as usize] = true;
    a[b'$' as usize] = true;
    a[b'&' as usize] = true;
    a[b'\'' as usize] = true;
    a[b'(' as usize] = true;
    a[b')' as usize] = true;
    a[b'*' as usize] = true;
    a[b'+' as usize] = true;
    a[b',' as usize] = true;
    a[b';' as usize] = true;
    a[b'=' as usize] = true;
}
pub(crate) const fn set_pchar(a: &mut [bool; 256]) {
    set_unreserved(a);
    set_sub_delims(a);
    a[b':' as usize] = true;
    a[b'@' as usize] = true;
}

pub const SCHEME_CHAR: [bool; 256] = {
    let mut a = [false; 256];
    set_alpha(&mut a);
    set_digit(&mut a);
    a[b'+' as usize] = true;
    a[b'-' as usize] = true;
    a[b'.' as usize] = true;
    a
};
pub const UNRESERVED_SUB_DELIMS: [bool; 256] = {
    let mut a = [false; 256];
    set_unreserved(&mut a);
    set_sub_delims(&mut a);
    a
};
pub const UNRESERVED_SUB_DELIMS_COLON: [bool; 256] = {
    let mut a = [false; 256];
    set_unreserved(&mut a);
    set_sub_delims(&mut a);
    a[b':' as usize] = true;
    a
};
pub const PCHAR: [bool; 256] = {
    let mut a = [false; 256];
    set_pchar(&mut a);
    a
};
pub const PCHAR_OR_SLASH: [bool; 256] = {
    let mut a = [false; 256];
    set_pchar(&mut a);
    a[b'/' as usize] = true;
    a
};
pub const PCHAR_OR_SLASH_OR_QUESTION: [bool; 256] = {
    let mut a = [false; 256];
    set_pchar(&mut a);
    a[b'/' as usize] = true;
    a[b'?' as usize] = true;
    a
};

#[inline]
pub fn is_ucschar(c: char) -> bool {
    matches!(c,
        '\u{A0}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}'
        | '\u{FDF0}'..='\u{FFEF}'
        | '\u{10000}'..='\u{1FFFD}'
        | '\u{20000}'..='\u{2FFFD}'
        | '\u{30000}'..='\u{3FFFD}'
        | '\u{40000}'..='\u{4FFFD}'
        | '\u{50000}'..='\u{5FFFD}'
        | '\u{60000}'..='\u{6FFFD}'
        | '\u{70000}'..='\u{7FFFD}'
        | '\u{80000}'..='\u{8FFFD}'
        | '\u{90000}'..='\u{9FFFD}'
        | '\u{A0000}'..='\u{AFFFD}'
        | '\u{B0000}'..='\u{BFFFD}'
        | '\u{C0000}'..='\u{CFFFD}'
        | '\u{D0000}'..='\u{DFFFD}'
        | '\u{E1000}'..='\u{EFFFD}')
}

#[inline]
pub fn is_iprivate(c: char) -> bool {
    matches!(c, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{FFFFD}' | '\u{100000}'..='\u{10FFFD}')
}

pub fn validate_scheme(scheme: &str) -> Result<(), IriParseError> {
    let bytes = scheme.as_bytes();
    let first = *bytes.first().ok_or(IriParseErrorKind::EmptyScheme)?;
    if !first.is_ascii_alphabetic() {
        return Err(IriParseErrorKind::InvalidSchemeCharacter(
            scheme.chars().next().unwrap_or_default(),
        )
        .into());
    }
    if let Some(i) = bytes[1..].iter().position(|&c| !SCHEME_CHAR[c as usize]) {
        return Err(IriParseErrorKind::InvalidSchemeCharacter(
            scheme[i + 1..].chars().next().unwrap_or_default(),
        )
        .into());
    }
    Ok(())
}

pub fn validate_authority(authority: &str, is_iri: bool) -> Result<(), IriParseError> {
    let Some(mut remaining) = authority.strip_prefix("//") else {
        return Err(IriParseErrorKind::InvalidHostCharacter(
            authority.chars().next().unwrap_or_default(),
        )
        .into());
    };
    if let Some(username_index) = memchr::memchr(b'@', remaining.as_bytes()) {
        let username = &remaining[..username_index];
        remaining = &remaining[username_index + 1..];
        validate_code_point_or_echar(username, UNRESERVED_SUB_DELIMS_COLON, is_iri, false)?;
    }
    if let Some(rest) = remaining.strip_prefix('[') {
        let Some(end) = memchr::memchr(b']', rest.as_bytes()) else {
            return Err(IriParseErrorKind::UnmatchedHostBracket.into());
        };
        validate_ip(&rest[..end])?;
        let rest = &rest[end + 1..];
        if !rest.is_empty() {
            let Some(port) = rest.strip_prefix(':') else {
                return Err(IriParseErrorKind::InvalidHostCharacter(
                    rest.chars().next().unwrap(),
                )
                .into());
            };
            validate_port(port)?;
        }
    } else {
        if let Some(port_i) = memchr::memchr(b':', remaining.as_bytes()) {
            validate_port(&remaining[port_i + 1..])?;
            remaining = &remaining[..port_i];
        }
        validate_code_point_or_echar(remaining, UNRESERVED_SUB_DELIMS, is_iri, false)?;
    }
    Ok(())
}

pub fn validate_host(host: &str, is_iri: bool) -> Result<(), IriParseError> {
    if let Some(ip) = host.strip_prefix('[') {
        let Some(end) = memchr::memchr(b']', ip.as_bytes()) else {
            return Err(IriParseErrorKind::UnmatchedHostBracket.into());
        };
        validate_ip(&ip[..end])?;
        if end + 1 != ip.len() {
            return Err(IriParseErrorKind::InvalidHostCharacter(
                ip[end + 1..].chars().next().unwrap_or_default(),
            )
            .into());
        }
        Ok(())
    } else {
        validate_code_point_or_echar(host, UNRESERVED_SUB_DELIMS, is_iri, false)
    }
}

pub fn validate_userinfo(s: &str, is_iri: bool) -> Result<(), IriParseError> {
    validate_code_point_or_echar(s, UNRESERVED_SUB_DELIMS_COLON, is_iri, false)
}

pub fn validate_ip(ip: &str) -> Result<(), IriParseError> {
    if ip.starts_with(['v', 'V']) {
        validate_ip_v_future(ip)
    } else {
        Ipv6Addr::from_str(ip)
            .map(|_| ())
            .map_err(|e| IriParseErrorKind::InvalidHostIp(e).into())
    }
}

pub fn validate_ip_v_future(ip: &str) -> Result<(), IriParseError> {
    let Some(rest) = ip.strip_prefix(['v', 'V']) else {
        return Err(IriParseErrorKind::InvalidHostCharacter(
            ip.chars().next().unwrap_or_default(),
        )
        .into());
    };
    let version_size = rest
        .as_bytes()
        .iter()
        .position(|c| !c.is_ascii_hexdigit())
        .unwrap_or(rest.len());
    if version_size == 0 {
        return Err(IriParseErrorKind::InvalidHostCharacter(
            rest.chars().next().unwrap_or_default(),
        )
        .into());
    }
    let rest = &rest[version_size..];
    let Some(rest) = rest.strip_prefix('.') else {
        return Err(IriParseErrorKind::InvalidHostCharacter(
            rest.chars().next().unwrap_or_default(),
        )
        .into());
    };
    if rest.is_empty() {
        return Err(IriParseErrorKind::InvalidHostCharacter(']').into());
    }
    if let Some(i) = rest
        .as_bytes()
        .iter()
        .position(|&c| !UNRESERVED_SUB_DELIMS_COLON[c as usize])
    {
        return Err(IriParseErrorKind::InvalidHostCharacter(
            rest[i..].chars().next().unwrap_or_default(),
        )
        .into());
    }
    Ok(())
}

pub fn validate_port(port: &str) -> Result<(), IriParseError> {
    if let Some(i) = port.as_bytes().iter().position(|c| !c.is_ascii_digit()) {
        return Err(IriParseErrorKind::InvalidPortCharacter(
            port[i..].chars().next().unwrap_or_default(),
        )
        .into());
    }
    Ok(())
}

pub fn validate_path(path: &str, is_iri: bool) -> Result<(), IriParseError> {
    validate_code_point_or_echar(path, PCHAR_OR_SLASH, is_iri, false)
}

pub fn validate_segment(seg: &str, is_iri: bool) -> Result<(), IriParseError> {
    validate_code_point_or_echar(seg, PCHAR, is_iri, false)
}

pub fn validate_query(query: &str, is_iri: bool) -> Result<(), IriParseError> {
    validate_code_point_or_echar(query, PCHAR_OR_SLASH_OR_QUESTION, is_iri, true)
}

pub fn validate_fragment(fragment: &str, is_iri: bool) -> Result<(), IriParseError> {
    validate_code_point_or_echar(fragment, PCHAR_OR_SLASH_OR_QUESTION, is_iri, false)
}

#[inline]
pub fn validate_code_point_or_echar(
    input: &str,
    ascii_validator: [bool; 256],
    is_iri: bool,
    allow_iprivate: bool,
) -> Result<(), IriParseError> {
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            if ascii_validator[b as usize] {
                i += 1;
            } else if b == b'%' {
                let c1 = bytes.get(i + 1).copied();
                let c2 = bytes.get(i + 2).copied();
                match (c1, c2) {
                    (Some(h1), Some(h2)) if h1.is_ascii_hexdigit() && h2.is_ascii_hexdigit() => {
                        i += 3;
                    }
                    _ => {
                        return Err(IriParseErrorKind::InvalidPercentEncoding([
                            Some('%'),
                            c1.map(|c| c as char),
                            c2.map(|c| c as char),
                        ])
                        .into());
                    }
                }
            } else {
                return Err(IriParseErrorKind::InvalidIriCodePoint(b as char).into());
            }
        } else if !is_iri {
            return Err(IriParseErrorKind::NonAsciiInUri.into());
        } else {
            let c = input[i..].chars().next().unwrap();
            if is_ucschar(c) || (allow_iprivate && is_iprivate(c)) {
                i += c.len_utf8();
            } else {
                return Err(IriParseErrorKind::InvalidIriCodePoint(c).into());
            }
        }
    }
    Ok(())
}

pub fn validate_iri_ref(iri: &str, p: Positions, is_iri: bool) -> Result<(), IriParseError> {
    if p.scheme_end > 0 {
        validate_scheme(&iri[..p.scheme_end - 1])?;
    }
    if p.authority_end > p.scheme_end {
        validate_authority(&iri[p.scheme_end..p.authority_end], is_iri)?;
    }
    validate_path(&iri[p.authority_end..p.path_end], is_iri)?;
    if p.query_end > p.path_end {
        validate_query(&iri[p.path_end + 1..p.query_end], is_iri)?;
    }
    if iri.len() > p.query_end {
        validate_fragment(&iri[p.query_end + 1..], is_iri)?;
    }
    Ok(())
}

pub fn validate_iri(iri: &str, p: Positions, is_iri: bool) -> Result<(), IriParseError> {
    if p.scheme_end == 0 {
        return Err(IriParseErrorKind::NoScheme.into());
    }
    validate_iri_ref(iri, p, is_iri)
}

pub fn validate_resolved_path(iri: &str, p: Positions) -> Result<(), IriParseError> {
    if p.scheme_end == p.authority_end && iri[p.authority_end..].starts_with("//") {
        return Err(IriParseErrorKind::PathStartingWithTwoSlashes.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_iri() {
        let s = "https://example.com/foo?q#f";
        let p = crate::parse::find_iri_positions(s);
        validate_iri(s, p, true).unwrap();
    }

    #[test]
    fn bad_scheme_empty() {
        assert!(validate_scheme("").is_err());
    }

    #[test]
    fn bad_scheme_digit_start() {
        assert!(validate_scheme("1ab").is_err());
    }

    #[test]
    fn pct_encoding_ok() {
        validate_path("/a%2Fb", false).unwrap();
    }

    #[test]
    fn pct_encoding_bad() {
        assert!(validate_path("/a%GG", false).is_err());
    }

    #[test]
    fn uri_rejects_unicode() {
        assert!(validate_path("/\u{00E9}", false).is_err());
    }

    #[test]
    fn iri_accepts_ucschar() {
        validate_path("/\u{00E9}", true).unwrap();
    }

    #[test]
    fn ipv6_host() {
        validate_host("[::1]", false).unwrap();
    }

    #[test]
    fn ipv6_host_bad() {
        assert!(validate_host("[::zzz]", false).is_err());
    }

    #[test]
    fn ipvfuture() {
        validate_host("[v1.abc]", false).unwrap();
    }

    #[test]
    fn iri_ref_relative() {
        let s = "../a/b";
        let p = crate::parse::find_iri_ref_positions(s);
        validate_iri_ref(s, p, true).unwrap();
    }

    #[test]
    fn query_iprivate_iri() {
        let s = "http://x/?\u{E000}";
        let p = crate::parse::find_iri_positions(s);
        validate_iri(s, p, true).unwrap();
    }
}
