use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs_core::Iri;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::hint::black_box;

#[path = "corpus.rs"]
mod corpus;
use corpus::IRI_CORPUS;

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
    p(
        "deep_dot",
        "http://a/./b/./c/../d/../e/./f/../../g",
        "http://a/e/g",
    ),
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
    pairs
        .iter()
        .map(|p| (p.label, Iri::parse(p.a).unwrap(), Iri::parse(p.b).unwrap()))
        .collect()
}

fn bench_iri_eq(c: &mut Criterion) {
    for (group, pairs) in [
        ("normalize/iri_eq/plain", PLAIN_PAIRS),
        ("normalize/iri_eq/pct", PCT_PAIRS),
        ("normalize/iri_eq/dot", DOT_PAIRS),
        ("normalize/iri_eq/scheme_case", SCHEME_CASE_PAIRS),
    ] {
        let parsed = parse_pairs(pairs);
        let mut g = c.benchmark_group(group);
        for (label, a, b) in &parsed {
            g.bench_with_input(BenchmarkId::from_parameter(label), &(a, b), |bh, (a, b)| {
                bh.iter(|| black_box(a) == black_box(b));
            });
        }
        g.finish();
    }
}

fn bench_iri_cmp(c: &mut Criterion) {
    for (group, pairs) in [
        ("normalize/iri_cmp/plain", PLAIN_PAIRS),
        ("normalize/iri_cmp/pct", PCT_PAIRS),
        ("normalize/iri_cmp/dot", DOT_PAIRS),
        ("normalize/iri_cmp/scheme_case", SCHEME_CASE_PAIRS),
    ] {
        let parsed = parse_pairs(pairs);
        let mut g = c.benchmark_group(group);
        for (label, a, b) in &parsed {
            g.bench_with_input(BenchmarkId::from_parameter(label), &(a, b), |bh, (a, b)| {
                bh.iter(|| black_box(a).cmp(black_box(b)));
            });
        }
        g.finish();
    }
}

fn bench_normalized_hash(c: &mut Criterion) {
    let iris: Vec<(&str, Iri<&str>)> = IRI_CORPUS
        .iter()
        .filter_map(|c| Iri::parse(c.input).ok().map(|i| (c.label, i)))
        .collect();

    let mut g = c.benchmark_group("normalize/normalized_hash");
    for (label, iri) in &iris {
        g.bench_with_input(BenchmarkId::from_parameter(label), iri, |b, iri| {
            b.iter(|| {
                let mut h = DefaultHasher::new();
                black_box(iri).hash(&mut h);
                h.finish()
            });
        });
    }
    g.finish();
}

criterion_group!(benches, bench_iri_eq, bench_iri_cmp, bench_normalized_hash);
criterion_main!(benches);
