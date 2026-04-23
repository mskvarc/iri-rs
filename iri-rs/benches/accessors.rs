use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs::{Iri, IriBuf};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    hint::black_box,
};

mod corpus;
use corpus::IRI_CORPUS;

fn bench(c: &mut Criterion) {
    let iris: Vec<(&str, &Iri)> = IRI_CORPUS.iter().filter_map(|c| Iri::new(c.input).ok().map(|i| (c.label, i))).collect();

    let mut g = c.benchmark_group("facade_accessors/iri_parts");
    for (label, iri) in &iris {
        g.bench_with_input(BenchmarkId::from_parameter(label), iri, |b, iri| {
            b.iter(|| {
                let parts = black_box(*iri).parts();
                parts.scheme.as_bytes().len()
                    + parts.authority.map(|x| x.as_bytes().len()).unwrap_or(0)
                    + parts.path.as_bytes().len()
                    + parts.query.map(|x| x.as_bytes().len()).unwrap_or(0)
                    + parts.fragment.map(|x| x.as_bytes().len()).unwrap_or(0)
            });
        });
    }
    g.finish();

    let bufs: Vec<(&str, IriBuf)> = IRI_CORPUS
        .iter()
        .filter_map(|c| IriBuf::new(c.input.to_string()).ok().map(|b| (c.label, b)))
        .collect();

    let mut g = c.benchmark_group("facade_accessors/hash");
    for (label, buf) in &bufs {
        g.bench_with_input(BenchmarkId::from_parameter(label), buf, |b, buf| {
            b.iter(|| {
                let mut h = DefaultHasher::new();
                black_box(buf).hash(&mut h);
                h.finish()
            });
        });
    }
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
