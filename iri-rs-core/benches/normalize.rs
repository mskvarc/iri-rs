use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs_core::{Iri, components::normalize_path};
use std::hint::black_box;

mod corpus;
use corpus::IRI_CORPUS;

fn bench_segment_iter(c: &mut Criterion) {
    let iris: Vec<(&str, Iri<&str>)> = IRI_CORPUS
        .iter()
        .filter_map(|c| Iri::parse(c.input).ok().map(|i| (c.label, i)))
        .collect();

    let mut g = c.benchmark_group("normalize/segments_iter");
    for (label, iri) in &iris {
        g.bench_with_input(BenchmarkId::from_parameter(label), iri, |b, iri| {
            b.iter(|| {
                let mut n = 0usize;
                for seg in black_box(iri).path_segments() {
                    n = n.wrapping_add(seg.as_bytes().len());
                }
                n
            });
        });
    }
    g.finish();

    let mut g = c.benchmark_group("normalize/path_normalized_owned");
    for (label, iri) in &iris {
        g.bench_with_input(BenchmarkId::from_parameter(label), iri, |b, iri| {
            b.iter(|| normalize_path(black_box(iri).path()));
        });
    }
    g.finish();
}

fn bench_pathological(c: &mut Criterion) {
    let evil = "http://a/x/./y/../z/./a/./b/../../c/./d/./e/../../../f/g/h/./i/j/../../k";
    let iri = Iri::parse(evil).unwrap();

    c.bench_function("normalize/pathological/owned", |b| {
        b.iter(|| normalize_path(black_box(&iri).path()));
    });
}

criterion_group!(benches, bench_segment_iter, bench_pathological);
criterion_main!(benches);
