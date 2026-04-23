use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs_core::validate::{
    validate_authority, validate_fragment, validate_path, validate_query,
};
use std::hint::black_box;

#[path = "corpus.rs"]
mod corpus;
use corpus::{IRI_CORPUS, URI_CORPUS};

struct Sample {
    label: &'static str,
    path: &'static str,
    query: &'static str,
    fragment: &'static str,
    authority: &'static str,
}

const fn s(
    label: &'static str,
    path: &'static str,
    query: &'static str,
    fragment: &'static str,
    authority: &'static str,
) -> Sample {
    Sample { label, path, query, fragment, authority }
}

const ASCII_SAMPLES: &[Sample] = &[
    s(
        "short",
        "/a/b/c",
        "q=1&r=2",
        "frag",
        "//user:pass@host.example:8443",
    ),
    s(
        "long",
        "/a/b/c/d/e/f/g/h/i/j/k/l/m/n.html",
        "a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8",
        "section-2",
        "//user:pass@host.example.longname:65535",
    ),
    s(
        "pct",
        "/%20%21%2F%3F%23",
        "q=%26reserved",
        "f%20rag",
        "//u%3Aer@host.example:80",
    ),
    s(
        "xl_path",
        "/aaaa/bbbb/cccc/dddd/eeee/ffff/gggg/hhhh/iiii/jjjj/kkkk/llll/mmmm/nnnn/oooo/pppp/qqqq/rrrr/ssss/tttt/uuuu/vvvv/wwww/xxxx/yyyy/zzzz/resource.html",
        "q=a+very+long+query+string+with+many+terms+and+symbols+%26+reserved",
        "section",
        "//host.example",
    ),
];

const UNICODE_SAMPLES: &[Sample] = &[
    s(
        "idn_jp",
        "/パス/ファイル",
        "q=パラメータ",
        "節",
        "//例え.テスト",
    ),
    s(
        "mixed",
        "/café/naïve",
        "q=hello&r=résumé",
        "frag",
        "//пример.рф",
    ),
];

fn bench_validate_components(c: &mut Criterion) {
    let mut g = c.benchmark_group("validate/path/uri");
    for sample in ASCII_SAMPLES {
        g.bench_with_input(BenchmarkId::from_parameter(sample.label), sample.path, |b, p| {
            b.iter(|| validate_path(black_box(p), false).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("validate/path/iri");
    for sample in ASCII_SAMPLES.iter().chain(UNICODE_SAMPLES.iter()) {
        g.bench_with_input(BenchmarkId::from_parameter(sample.label), sample.path, |b, p| {
            b.iter(|| validate_path(black_box(p), true).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("validate/query/uri");
    for sample in ASCII_SAMPLES {
        g.bench_with_input(BenchmarkId::from_parameter(sample.label), sample.query, |b, q| {
            b.iter(|| validate_query(black_box(q), false).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("validate/query/iri");
    for sample in ASCII_SAMPLES.iter().chain(UNICODE_SAMPLES.iter()) {
        g.bench_with_input(BenchmarkId::from_parameter(sample.label), sample.query, |b, q| {
            b.iter(|| validate_query(black_box(q), true).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("validate/fragment/uri");
    for sample in ASCII_SAMPLES {
        g.bench_with_input(BenchmarkId::from_parameter(sample.label), sample.fragment, |b, f| {
            b.iter(|| validate_fragment(black_box(f), false).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("validate/fragment/iri");
    for sample in ASCII_SAMPLES.iter().chain(UNICODE_SAMPLES.iter()) {
        g.bench_with_input(BenchmarkId::from_parameter(sample.label), sample.fragment, |b, f| {
            b.iter(|| validate_fragment(black_box(f), true).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("validate/authority/uri");
    for sample in ASCII_SAMPLES {
        g.bench_with_input(
            BenchmarkId::from_parameter(sample.label),
            sample.authority,
            |b, a| {
                b.iter(|| validate_authority(black_box(a), false).ok());
            },
        );
    }
    g.finish();

    let mut g = c.benchmark_group("validate/authority/iri");
    for sample in ASCII_SAMPLES.iter().chain(UNICODE_SAMPLES.iter()) {
        g.bench_with_input(
            BenchmarkId::from_parameter(sample.label),
            sample.authority,
            |b, a| {
                b.iter(|| validate_authority(black_box(a), true).ok());
            },
        );
    }
    g.finish();
}

fn bench_validate_corpus_paths(c: &mut Criterion) {
    let iri_paths: Vec<(&str, &str)> = IRI_CORPUS
        .iter()
        .map(|c| (c.label, extract_path(c.input)))
        .collect();
    let uri_paths: Vec<(&str, &str)> = URI_CORPUS
        .iter()
        .map(|c| (c.label, extract_path(c.input)))
        .collect();

    let mut g = c.benchmark_group("validate/corpus_path/iri");
    for (label, path) in &iri_paths {
        g.bench_with_input(BenchmarkId::from_parameter(label), *path, |b, p| {
            b.iter(|| validate_path(black_box(p), true).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("validate/corpus_path/uri");
    for (label, path) in &uri_paths {
        g.bench_with_input(BenchmarkId::from_parameter(label), *path, |b, p| {
            b.iter(|| validate_path(black_box(p), false).ok());
        });
    }
    g.finish();
}

fn extract_path(iri: &str) -> &str {
    // Rough path slice: after scheme://authority, up to ? or #.
    let rest = match iri.find("://") {
        Some(i) => &iri[i + 3..],
        None => iri,
    };
    let after_auth = match rest.as_bytes().iter().position(|&b| b == b'/') {
        Some(i) => &rest[i..],
        None => "",
    };
    let end = after_auth
        .as_bytes()
        .iter()
        .position(|&b| b == b'?' || b == b'#')
        .unwrap_or(after_auth.len());
    &after_auth[..end]
}

criterion_group!(benches, bench_validate_components, bench_validate_corpus_paths);
criterion_main!(benches);
