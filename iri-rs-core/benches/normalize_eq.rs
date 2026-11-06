use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs_core::Iri;
use std::hint::black_box;

struct Pair {
    label: &'static str,
    a: &'static str,
    b: &'static str,
}

const fn p(label: &'static str, a: &'static str, b: &'static str) -> Pair {
    Pair { label, a, b }
}

const PLAIN_PAIRS: &[Pair] = &[
    p("root_eq", "http://example.com/", "http://example.com/"),
    p(
        "long_path_eq",
        "http://example.com/a/b/c/d/e/f/g/h/i/j/k/l/m/n.html",
        "http://example.com/a/b/c/d/e/f/g/h/i/j/k/l/m/n.html",
    ),
    p(
        "query_eq",
        "http://example.com/p?a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8",
        "http://example.com/p?a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8",
    ),
    p(
        "last_seg_ne",
        "http://example.com/a/b/c/d/e/f/g/h/i/j/k/l/m/n.html",
        "http://example.com/a/b/c/d/e/f/g/h/i/j/k/l/m/n.htmX",
    ),
];

const PCT_PAIRS: &[Pair] = &[
    p("tilde_encoded", "http://a/%7Eb", "http://a/~b"),
    p("hex_case", "http://a/%2f", "http://a/%2F"),
    p(
        "long_pct",
        "http://example.com/%E4%B8%AD%E6%96%87?q=%E6%B5%8B%E8%AF%95",
        "http://example.com/%E4%B8%AD%E6%96%87?q=%E6%B5%8B%E8%AF%95",
    ),
];

const DOT_PAIRS: &[Pair] = &[
    p("shallow_dot", "http://a/b/c/./../../g", "http://a/g"),
    p("deep_dot", "http://a/./b/./c/../d/../e/./f/../../g", "http://a/e/g"),
];

const SCHEME_CASE_PAIRS: &[Pair] = &[
    p("http_vs_HTTP", "http://example.com/", "HTTP://example.com/"),
    p(
        "https_mixed",
        "HttPs://user:pass@host.example:8443/x/y",
        "https://user:pass@host.example:8443/x/y",
    ),
];

fn parse_pairs(pairs: &[Pair]) -> Vec<(&'static str, Iri<&'static str>, Iri<&'static str>)> {
    pairs.iter().map(|p| (p.label, Iri::parse(p.a).unwrap(), Iri::parse(p.b).unwrap())).collect()
}

fn bench_equivalent(c: &mut Criterion) {
    for (group, pairs) in [
        ("normalize/equivalent/plain", PLAIN_PAIRS),
        ("normalize/equivalent/pct", PCT_PAIRS),
        ("normalize/equivalent/dot", DOT_PAIRS),
        ("normalize/equivalent/scheme_case", SCHEME_CASE_PAIRS),
    ] {
        let parsed = parse_pairs(pairs);
        let mut g = c.benchmark_group(group);
        for (label, a, b) in &parsed {
            g.bench_with_input(BenchmarkId::from_parameter(label), &(a, b), |bh, (a, b)| {
                bh.iter(|| black_box(a).equivalent(black_box(b)));
            });
        }
        g.finish();
    }
}

criterion_group!(benches, bench_equivalent);
criterion_main!(benches);
