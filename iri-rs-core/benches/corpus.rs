#![allow(dead_code)]

// Input corpus used across benches. Separate slices so URI benches can skip
// IRI-only (non-ASCII) inputs.

pub struct Case {
    pub label: &'static str,
    pub input: &'static str,
}

const fn c(label: &'static str, input: &'static str) -> Case {
    Case { label, input }
}

/// IRIs and IRI references valid as IRIs (may contain non-ASCII code points).
pub const IRI_CORPUS: &[Case] = &[
    c("root", "http://example.com/"),
    c("short_path", "http://example.com/a/b/c"),
    c("long_path", "http://example.com/a/b/c/d/e/f/g/h/i/j/k/l/m/n.html"),
    c(
        "xl_path",
        "http://example.com/aaaa/bbbb/cccc/dddd/eeee/ffff/gggg/hhhh/iiii/jjjj/kkkk/llll/mmmm/nnnn/oooo/pppp/qqqq/rrrr/ssss/tttt/uuuu/vvvv/wwww/xxxx/yyyy/zzzz/resource.html",
    ),
    c("query_many", "http://example.com/p?a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8"),
    c(
        "query_long",
        "http://example.com/search?q=a+very+long+query+string+with+many+terms+and+symbols+%26+reserved",
    ),
    c("pct_ascii", "http://example.com/%20%21%2F%3F%23"),
    c("pct_utf8", "http://example.com/%E4%B8%AD%E6%96%87?q=%E6%B5%8B%E8%AF%95"),
    c("idn_jp", "http://例え.テスト/パス/ファイル"),
    c("idn_cyr", "http://пример.рф/путь"),
    c("userinfo_port", "https://user:pass@host.example:8443/x/y"),
    c("ipv4", "http://192.0.2.17:80/a"),
    c("ipv6", "http://[2001:db8::1]:443/api"),
    c("fragment_only_host", "http://a.b/c#frag"),
    c("empty_path_host", "http://example.com"),
    c("dot_shallow", "http://a/b/c/./../../g"),
    c("dot_deep", "http://a/./b/./c/../d/../e/./f/../../g"),
    c("file", "file:///etc/hosts"),
    c("urn_isbn", "urn:isbn:0451450523"),
    c("urn_uuid", "urn:uuid:f47ac10b-58cc-4372-a567-0e02b2c3d479"),
    c("mailto", "mailto:user@example.com?subject=hi"),
    c("data_b64", "data:text/plain;base64,SGVsbG8="),
    c("tag", "tag:example.com,2026-01-01:foo/bar"),
    c("scheme_only", "about:"),
];

/// Subset of `IRI_CORPUS` that is also a valid URI (ASCII only).
pub const URI_CORPUS: &[Case] = &[
    c("root", "http://example.com/"),
    c("short_path", "http://example.com/a/b/c"),
    c("long_path", "http://example.com/a/b/c/d/e/f/g/h/i/j/k/l/m/n.html"),
    c(
        "xl_path",
        "http://example.com/aaaa/bbbb/cccc/dddd/eeee/ffff/gggg/hhhh/iiii/jjjj/kkkk/llll/mmmm/nnnn/oooo/pppp/qqqq/rrrr/ssss/tttt/uuuu/vvvv/wwww/xxxx/yyyy/zzzz/resource.html",
    ),
    c("query_many", "http://example.com/p?a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8"),
    c(
        "query_long",
        "http://example.com/search?q=a+very+long+query+string+with+many+terms+and+symbols+%26+reserved",
    ),
    c("pct_ascii", "http://example.com/%20%21%2F%3F%23"),
    c("pct_utf8", "http://example.com/%E4%B8%AD%E6%96%87?q=%E6%B5%8B%E8%AF%95"),
    c("userinfo_port", "https://user:pass@host.example:8443/x/y"),
    c("ipv4", "http://192.0.2.17:80/a"),
    c("ipv6", "http://[2001:db8::1]:443/api"),
    c("fragment_only_host", "http://a.b/c#frag"),
    c("empty_path_host", "http://example.com"),
    c("dot_shallow", "http://a/b/c/./../../g"),
    c("dot_deep", "http://a/./b/./c/../d/../e/./f/../../g"),
    c("file", "file:///etc/hosts"),
    c("urn_isbn", "urn:isbn:0451450523"),
    c("urn_uuid", "urn:uuid:f47ac10b-58cc-4372-a567-0e02b2c3d479"),
    c("mailto", "mailto:user@example.com?subject=hi"),
    c("data_b64", "data:text/plain;base64,SGVsbG8="),
    c("tag", "tag:example.com,2026-01-01:foo/bar"),
    c("scheme_only", "about:"),
];

/// IRI/URI references (no scheme OK). Used for reference-form benches.
pub const REF_CORPUS: &[Case] = &[
    c("rel_simple", "a/b/c"),
    c("rel_dot", "./a/b"),
    c("rel_dotdot", "../x/y"),
    c("rel_query", "?q=1&r=2"),
    c("rel_fragment", "#section-2"),
    c("rel_authority", "//other.example/p"),
    c("rel_empty", ""),
    c("abs_root", "/a/b/c"),
    c("abs_iri", "http://example.com/"),
];

/// Strings that must fail validation. Each should be rejected on the first
/// illegal byte.
pub const INVALID: &[Case] = &[
    c("space_in_path", "http://exa mple.com/p"),
    c("unterminated_ipv6", "http://[2001:db8::1/p"),
    c("bad_scheme_start", "1ttp://example.com/"),
    c("bare_text", "this is not an iri"),
    c("lone_percent", "http://example.com/%"),
    c("short_percent", "http://example.com/%2"),
    c("empty_scheme", "://example.com/"),
    c("illegal_char", "http://example.com/\u{0001}"),
];

/// RFC 3986 §5.4 base for resolution.
pub const RFC3986_BASE: &str = "http://a/b/c/d;p?q";

/// RFC 3986 §5.4.1 normal resolution pairs — (reference, expected).
pub const RFC3986_NORMAL: &[(&str, &str)] = &[
    ("g:h", "g:h"),
    ("g", "http://a/b/c/g"),
    ("./g", "http://a/b/c/g"),
    ("g/", "http://a/b/c/g/"),
    ("/g", "http://a/g"),
    ("//g", "http://g"),
    ("?y", "http://a/b/c/d;p?y"),
    ("g?y", "http://a/b/c/g?y"),
    ("#s", "http://a/b/c/d;p?q#s"),
    ("g#s", "http://a/b/c/g#s"),
    ("g?y#s", "http://a/b/c/g?y#s"),
    (";x", "http://a/b/c/;x"),
    ("g;x", "http://a/b/c/g;x"),
    ("g;x?y#s", "http://a/b/c/g;x?y#s"),
    ("", "http://a/b/c/d;p?q"),
    (".", "http://a/b/c/"),
    ("./", "http://a/b/c/"),
    ("..", "http://a/b/"),
    ("../", "http://a/b/"),
    ("../g", "http://a/b/g"),
    ("../..", "http://a/"),
    ("../../", "http://a/"),
    ("../../g", "http://a/g"),
];

/// RFC 3986 §5.4.2 abnormal resolution pairs — these exercise dot-segment edge
/// cases.
pub const RFC3986_ABNORMAL: &[(&str, &str)] = &[
    ("../../../g", "http://a/g"),
    ("../../../../g", "http://a/g"),
    ("/./g", "http://a/g"),
    ("/../g", "http://a/g"),
    ("g.", "http://a/b/c/g."),
    (".g", "http://a/b/c/.g"),
    ("g..", "http://a/b/c/g.."),
    ("..g", "http://a/b/c/..g"),
    ("./../g", "http://a/b/g"),
    ("./g/.", "http://a/b/c/g/"),
    ("g/./h", "http://a/b/c/g/h"),
    ("g/../h", "http://a/b/c/h"),
    ("g;x=1/./y", "http://a/b/c/g;x=1/y"),
    ("g;x=1/../y", "http://a/b/c/y"),
    ("g?y/./x", "http://a/b/c/g?y/./x"),
    ("g?y/../x", "http://a/b/c/g?y/../x"),
    ("g#s/./x", "http://a/b/c/g#s/./x"),
    ("g#s/../x", "http://a/b/c/g#s/../x"),
];
