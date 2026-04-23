#![allow(dead_code)]

// Input corpus used across facade benches. Kept deliberately in sync with the
// core benches so regressions in the re-export layer show up with identical
// labels.

pub struct Case {
    pub label: &'static str,
    pub input: &'static str,
}

const fn c(label: &'static str, input: &'static str) -> Case {
    Case { label, input }
}

pub const IRI_CORPUS: &[Case] = &[
    c("root", "http://example.com/"),
    c("short_path", "http://example.com/a/b/c"),
    c("long_path", "http://example.com/a/b/c/d/e/f/g/h/i/j/k/l/m/n.html"),
    c("query_many", "http://example.com/p?a=1&b=2&c=3&d=4&e=5"),
    c("pct_utf8", "http://example.com/%E4%B8%AD%E6%96%87"),
    c("idn_jp", "http://例え.テスト/パス"),
    c("userinfo_port", "https://user:pass@host.example:8443/x"),
    c("ipv6", "http://[2001:db8::1]:443/api"),
    c("dot_deep", "http://a/./b/./c/../d/../e/./f/../../g"),
    c("file", "file:///etc/hosts"),
    c("urn_uuid", "urn:uuid:f47ac10b-58cc-4372-a567-0e02b2c3d479"),
    c("mailto", "mailto:user@example.com?subject=hi"),
    c("data_b64", "data:text/plain;base64,SGVsbG8="),
];

pub const URI_CORPUS: &[Case] = &[
    c("root", "http://example.com/"),
    c("short_path", "http://example.com/a/b/c"),
    c("long_path", "http://example.com/a/b/c/d/e/f/g/h/i/j/k/l/m/n.html"),
    c("query_many", "http://example.com/p?a=1&b=2&c=3&d=4&e=5"),
    c("pct_utf8", "http://example.com/%E4%B8%AD%E6%96%87"),
    c("userinfo_port", "https://user:pass@host.example:8443/x"),
    c("ipv6", "http://[2001:db8::1]:443/api"),
    c("dot_deep", "http://a/./b/./c/../d/../e/./f/../../g"),
    c("file", "file:///etc/hosts"),
    c("urn_uuid", "urn:uuid:f47ac10b-58cc-4372-a567-0e02b2c3d479"),
];

pub const INVALID: &[Case] = &[
    c("space_in_path", "http://exa mple.com/p"),
    c("unterminated_ipv6", "http://[2001:db8::1/p"),
    c("bad_scheme_start", "1ttp://example.com/"),
    c("bare_text", "this is not an iri"),
    c("lone_percent", "http://example.com/%"),
];

pub const RFC3986_BASE: &str = "http://a/b/c/d;p?q";

pub const RFC3986_PAIRS: &[(&str, &str)] = &[
    ("g:h", "g:h"),
    ("g", "http://a/b/c/g"),
    ("./g", "http://a/b/c/g"),
    ("/g", "http://a/g"),
    ("//g", "http://g"),
    ("?y", "http://a/b/c/d;p?y"),
    ("g?y#s", "http://a/b/c/g?y#s"),
    ("..", "http://a/b/"),
    ("../g", "http://a/b/g"),
    ("../../../g", "http://a/g"),
    ("g;x=1/../y", "http://a/b/c/y"),
    ("g?y/../x", "http://a/b/c/g?y/../x"),
];
