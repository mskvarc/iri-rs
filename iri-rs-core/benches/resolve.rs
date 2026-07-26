use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs_core::{Iri, IriRef, IriRefBuf};
use std::hint::black_box;

mod corpus;
use corpus::{RFC3986_ABNORMAL, RFC3986_BASE, RFC3986_NORMAL};

fn bench_resolved(c: &mut Criterion) {
    let base = Iri::parse(RFC3986_BASE).unwrap();

    let mut g = c.benchmark_group("resolve/rfc3986_normal");
    for (r, _) in RFC3986_NORMAL {
        let rref = IriRef::parse(*r).unwrap();
        g.bench_with_input(BenchmarkId::from_parameter(r), &rref, |b, rref| {
            b.iter(|| black_box(rref).resolved(black_box(&base)));
        });
    }
    g.finish();

    let mut g = c.benchmark_group("resolve/rfc3986_abnormal");
    for (r, _) in RFC3986_ABNORMAL {
        let rref = IriRef::parse(*r).unwrap();
        g.bench_with_input(BenchmarkId::from_parameter(r), &rref, |b, rref| {
            b.iter(|| black_box(rref).resolved(black_box(&base)));
        });
    }
    g.finish();
}

/// Deep base mirroring the JSON-LD `@base` shape that dominates real workloads.
const DEEP_BASE: &str = "http://example.com/some/deep/directory/and/file";

/// (label, base, reference) — realistic references exercising each resolver
/// branch with representative path lengths and `.` placement.
const REALISTIC: &[(&str, &str, &str)] = &[
    ("abs_clean", RFC3986_BASE, "https://api.example.com/ngsi-ld/v1/entities/urn:ngsi-ld:Device:001"),
    ("abs_ext", RFC3986_BASE, "https://cdn.example.com/assets/js/app.bundle.min.js"),
    ("net_ext", RFC3986_BASE, "//cdn.example.com/assets/app.min.js"),
    ("rel_deep_clean", DEEP_BASE, "other-resource"),
    ("rel_deep_ext", DEEP_BASE, "other.file.v2.json"),
    ("abs_path_long_clean", DEEP_BASE, "/api/v2/collections/items/12345/attributes/temperature"),
];

fn bench_realistic(c: &mut Criterion) {
    let mut g = c.benchmark_group("resolve/realistic");
    for (label, base, r) in REALISTIC {
        let base = Iri::parse(*base).unwrap();
        let rref = IriRef::parse(*r).unwrap();
        g.bench_with_input(BenchmarkId::from_parameter(label), &rref, |b, rref| {
            b.iter(|| black_box(rref).resolved(black_box(&base)));
        });
    }
    g.finish();
}

fn bench_in_place(c: &mut Criterion) {
    let base = Iri::parse(RFC3986_BASE).unwrap();

    let mut g = c.benchmark_group("resolve/in_place");
    for (r, _) in RFC3986_NORMAL.iter().chain(RFC3986_ABNORMAL.iter()) {
        let seed = IriRefBuf::new(r.to_string()).unwrap();
        g.bench_with_input(BenchmarkId::from_parameter(r), &seed, |b, seed| {
            b.iter_batched_ref(
                || seed.clone(),
                |buf| {
                    let _ = buf.resolve(black_box(&base));
                },
                BatchSize::SmallInput,
            );
        });
    }
    g.finish();
}

criterion_group!(benches, bench_resolved, bench_realistic, bench_in_place);
criterion_main!(benches);
