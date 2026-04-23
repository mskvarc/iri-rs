use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs_core::Iri;
use std::hint::black_box;

mod corpus;
use corpus::IRI_CORPUS;

fn bench_segment_iter(c: &mut Criterion) {
    let iris: Vec<(&str, &Iri)> = IRI_CORPUS.iter().filter_map(|c| Iri::new(c.input).ok().map(|i| (c.label, i))).collect();

    // Raw segment iteration (no dot-segment removal).
    let mut g = c.benchmark_group("normalize/segments_iter");
    for (label, iri) in &iris {
        g.bench_with_input(BenchmarkId::from_parameter(label), iri, |b, iri| {
            b.iter(|| {
                let mut n = 0usize;
                for seg in black_box(*iri).path().segments() {
                    n = n.wrapping_add(seg.as_bytes().len());
                }
                n
            });
        });
    }
    g.finish();

    // Normalised iteration (dot-segment resolution).
    let mut g = c.benchmark_group("normalize/normalized_segments_iter");
    for (label, iri) in &iris {
        g.bench_with_input(BenchmarkId::from_parameter(label), iri, |b, iri| {
            b.iter(|| {
                let mut n = 0usize;
                for seg in black_box(*iri).path().normalized_segments() {
                    n = n.wrapping_add(seg.as_bytes().len());
                }
                n
            });
        });
    }
    g.finish();
}

fn bench_normalize_owned(c: &mut Criterion) {
    let iris: Vec<(&str, &Iri)> = IRI_CORPUS.iter().filter_map(|c| Iri::new(c.input).ok().map(|i| (c.label, i))).collect();

    // Allocating normalise — returns `PathBuf`.
    let mut g = c.benchmark_group("normalize/path_normalized_owned");
    for (label, iri) in &iris {
        g.bench_with_input(BenchmarkId::from_parameter(label), iri, |b, iri| {
            b.iter(|| black_box(*iri).path().normalized());
        });
    }
    g.finish();
}

fn bench_pathological(c: &mut Criterion) {
    // Stress case: long string of dot-segments plus regular segments.
    let evil = "http://a/x/./y/../z/./a/./b/../../c/./d/./e/../../../f/g/h/./i/j/../../k";
    let iri = Iri::new(evil).unwrap();

    c.bench_function("normalize/pathological/segments", |b| {
        b.iter(|| {
            let mut n = 0usize;
            for seg in black_box(iri).path().normalized_segments() {
                n = n.wrapping_add(seg.as_bytes().len());
            }
            n
        });
    });

    c.bench_function("normalize/pathological/owned", |b| {
        b.iter(|| black_box(iri).path().normalized());
    });
}

criterion_group!(benches, bench_segment_iter, bench_normalize_owned, bench_pathological);
criterion_main!(benches);
