//! Position finders — oxiri port. Locate scheme/authority/path/query/fragment boundaries.
use memchr::{memchr, memchr2, memchr3};

/// Cached component boundaries inside an IRI/URI string.
///
/// Indices are byte offsets.
/// * `scheme_end` — index one past the `:` after the scheme (0 if no scheme)
/// * `authority_end` — index one past the last byte of the authority (equals `scheme_end` if none)
/// * `path_end` — index one past the last byte of the path
/// * `query_end` — index one past the last byte of the query (equals `path_end` if none)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Positions {
    pub scheme_end: usize,
    pub authority_end: usize,
    pub path_end: usize,
    pub query_end: usize,
}

impl Positions {
    pub const EMPTY: Self = Self {
        scheme_end: 0,
        authority_end: 0,
        path_end: 0,
        query_end: 0,
    };
}

/// Finds positions assuming the input is a full IRI (absolute or relative, scheme allowed).
#[inline]
pub fn find_iri_positions(iri: &str) -> Positions {
    let iri = iri.as_bytes();
    let scheme_end = memchr(b':', iri).map_or(0, |l| l + 1);
    find_iri_positions_knowing_scheme_end(iri, scheme_end)
}

#[inline]
pub fn find_iri_positions_knowing_scheme_end(iri: &[u8], scheme_end: usize) -> Positions {
    let path_end = memchr2(b'?', b'#', &iri[scheme_end..]).map_or(iri.len(), |l| scheme_end + l);
    let query_end = memchr(b'#', &iri[path_end..]).map_or(iri.len(), |l| path_end + l);
    let authority_end = if scheme_end + 2 <= path_end
        && iri[scheme_end] == b'/'
        && iri[scheme_end + 1] == b'/'
    {
        memchr(b'/', &iri[scheme_end + 2..path_end]).map_or(path_end, |l| scheme_end + 2 + l)
    } else {
        scheme_end
    };
    Positions {
        scheme_end,
        authority_end,
        path_end,
        query_end,
    }
}

/// Finds positions for an IRI reference (may lack scheme, start with `/`, `?`, `#`, or be empty).
pub fn find_iri_ref_positions(iri: &str) -> Positions {
    let iri = iri.as_bytes();
    match iri.first().copied() {
        Some(b'/') => find_iri_positions_knowing_scheme_end(iri, 0),
        Some(b'?') => {
            let query_end = memchr(b'#', iri).unwrap_or(iri.len());
            Positions {
                scheme_end: 0,
                authority_end: 0,
                path_end: 0,
                query_end,
            }
        }
        Some(b'#') | None => Positions::EMPTY,
        _ => {
            let scheme_end = memchr3(b':', b'?', b'/', iri).map_or(0, |index| {
                if iri[index] == b':' {
                    if memchr(b'#', &iri[..index]).is_some() {
                        0
                    } else {
                        index + 1
                    }
                } else {
                    0
                }
            });
            find_iri_positions_knowing_scheme_end(iri, scheme_end)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iri_ref_empty() {
        assert_eq!(find_iri_ref_positions(""), Positions::EMPTY);
    }

    #[test]
    fn iri_ref_fragment_only() {
        assert_eq!(find_iri_ref_positions("#frag"), Positions::EMPTY);
    }

    #[test]
    fn iri_ref_query_only() {
        let p = find_iri_ref_positions("?q");
        assert_eq!(
            p,
            Positions {
                scheme_end: 0,
                authority_end: 0,
                path_end: 0,
                query_end: 2
            }
        );
    }

    #[test]
    fn iri_full() {
        let s = "http://host/p?q#f";
        let p = find_iri_positions(s);
        assert_eq!(&s[..p.scheme_end - 1], "http");
        assert_eq!(&s[p.scheme_end + 2..p.authority_end], "host");
        assert_eq!(&s[p.authority_end..p.path_end], "/p");
        assert_eq!(&s[p.path_end + 1..p.query_end], "q");
        assert_eq!(&s[p.query_end + 1..], "f");
    }

    #[test]
    fn iri_ref_path_only() {
        let p = find_iri_ref_positions("/a/b");
        assert_eq!(p.scheme_end, 0);
        assert_eq!(p.authority_end, 0);
        assert_eq!(p.path_end, 4);
    }

    #[test]
    fn iri_ref_with_scheme() {
        let p = find_iri_ref_positions("mailto:a@b");
        assert_eq!(p.scheme_end, 7);
    }

    #[test]
    fn iri_fragment_before_colon_not_scheme() {
        let p = find_iri_ref_positions("a#b:c");
        assert_eq!(p.scheme_end, 0);
    }
}
