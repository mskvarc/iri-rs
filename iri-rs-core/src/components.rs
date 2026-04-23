//! Helper views over path / authority components, and convenience accessors.

use std::ops::Deref;

use crate::types::{Iri, IriRef, Uri, UriRef};

#[inline]
pub fn path_segments(path: &str) -> PathSegments<'_> {
    let rest = path.strip_prefix('/').unwrap_or(path);
    PathSegments {
        inner: if rest.is_empty() {
            None
        } else {
            Some(rest.split('/'))
        },
    }
}

pub struct PathSegments<'a> {
    inner: Option<std::str::Split<'a, char>>,
}

impl<'a> Iterator for PathSegments<'a> {
    type Item = &'a str;
    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        self.inner.as_mut()?.next()
    }
}

impl<'a> DoubleEndedIterator for PathSegments<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a str> {
        self.inner.as_mut()?.next_back()
    }
}

#[inline]
pub fn path_is_absolute(path: &str) -> bool {
    path.starts_with('/')
}

pub fn normalize_path(path: &str) -> String {
    let mut input = path;
    let mut out = String::with_capacity(path.len());

    while !input.is_empty() {
        if let Some(rest) = input.strip_prefix("../") {
            input = rest;
        } else if let Some(rest) = input.strip_prefix("./") {
            input = rest;
        } else if input.starts_with("/./") {
            input = &input[2..];
        } else if input == "/." {
            input = "/";
        } else if input.starts_with("/../") {
            input = &input[3..];
            pop_to_slash(&mut out);
        } else if input == "/.." {
            input = "/";
            pop_to_slash(&mut out);
        } else if input == "." || input == ".." {
            input = "";
        } else {
            let rest = if let Some(r) = input.strip_prefix('/') {
                out.push('/');
                r
            } else {
                input
            };
            let end = memchr::memchr(b'/', rest.as_bytes()).unwrap_or(rest.len());
            out.push_str(&rest[..end]);
            input = &rest[end..];
        }
    }
    out
}

fn pop_to_slash(out: &mut String) {
    if let Some(i) = memchr::memrchr(b'/', out.as_bytes()) {
        out.truncate(i);
    } else {
        out.clear();
    }
}

pub fn split_authority(authority: &str) -> (Option<&str>, &str, Option<&str>) {
    let (user_info, rest) = match memchr::memchr(b'@', authority.as_bytes()) {
        Some(i) => (Some(&authority[..i]), &authority[i + 1..]),
        None => (None, authority),
    };
    let (host, port) = if let Some(rest_in) = rest.strip_prefix('[') {
        if let Some(end) = memchr::memchr(b']', rest_in.as_bytes()) {
            let host = &rest[..end + 2];
            let tail = &rest[end + 2..];
            match tail.strip_prefix(':') {
                Some(p) => (host, Some(p)),
                None => (host, None),
            }
        } else {
            (rest, None)
        }
    } else {
        match memchr::memchr(b':', rest.as_bytes()) {
            Some(i) => (&rest[..i], Some(&rest[i + 1..])),
            None => (rest, None),
        }
    };
    (user_info, host, port)
}

impl<T: Deref<Target = str>> Iri<T> {
    pub fn path_segments(&self) -> PathSegments<'_> {
        path_segments(self.path())
    }

    pub fn authority_parts(&self) -> Option<(Option<&str>, &str, Option<&str>)> {
        self.authority().map(split_authority)
    }
}

impl<T: Deref<Target = str>> IriRef<T> {
    pub fn path_segments(&self) -> PathSegments<'_> {
        path_segments(self.path())
    }
    pub fn authority_parts(&self) -> Option<(Option<&str>, &str, Option<&str>)> {
        self.authority().map(split_authority)
    }
}

impl<T: Deref<Target = str>> Uri<T> {
    pub fn path_segments(&self) -> PathSegments<'_> {
        path_segments(self.path())
    }
    pub fn authority_parts(&self) -> Option<(Option<&str>, &str, Option<&str>)> {
        self.authority().map(split_authority)
    }
}

impl<T: Deref<Target = str>> UriRef<T> {
    pub fn path_segments(&self) -> PathSegments<'_> {
        path_segments(self.path())
    }
    pub fn authority_parts(&self) -> Option<(Option<&str>, &str, Option<&str>)> {
        self.authority().map(split_authority)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_absolute() {
        let segs: Vec<&str> = path_segments("/a/b/c").collect();
        assert_eq!(segs, vec!["a", "b", "c"]);
    }

    #[test]
    fn segments_relative() {
        let segs: Vec<&str> = path_segments("a/b/c").collect();
        assert_eq!(segs, vec!["a", "b", "c"]);
    }

    #[test]
    fn segments_trailing_slash() {
        let segs: Vec<&str> = path_segments("/a/b/").collect();
        assert_eq!(segs, vec!["a", "b", ""]);
    }

    #[test]
    fn split_auth_host_only() {
        assert_eq!(split_authority("example.com"), (None, "example.com", None));
    }

    #[test]
    fn split_auth_full() {
        assert_eq!(
            split_authority("user:pass@host:80"),
            (Some("user:pass"), "host", Some("80"))
        );
    }

    #[test]
    fn split_auth_ipv6() {
        assert_eq!(split_authority("[::1]:8080"), (None, "[::1]", Some("8080")));
    }

    #[test]
    fn normalize_basic() {
        assert_eq!(normalize_path("/a/b/../c"), "/a/c");
        assert_eq!(normalize_path("a/./b/../c"), "a/c");
    }
}
