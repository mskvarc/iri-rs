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
    let iris: Vec<(&str, Iri<&str>)> = IRI_CORPUS
        .iter()
        .filter_map(|c| Iri::parse(c.input).ok().map(|i| (c.label, i)))
        .collect();

    let mut g = c.benchmark_group("facade_accessors/iri_fields");
    for (label, iri) in &iris {
        g.bench_with_input(BenchmarkId::from_parameter(label), iri, |b, iri| {
            b.iter(|| {
                let iri = black_box(*iri);
                iri.scheme().len()
                    + iri.authority().map(|x| x.len()).unwrap_or(0)
                    + iri.path().len()
                    + iri.query().map(|x| x.len()).unwrap_or(0)
                    + iri.fragment().map(|x| x.len()).unwrap_or(0)
            });
        });
    }
    g.finish();

    let bufs: Vec<(&str, IriBuf)> = IRI_CORPUS
        .iter()
        .filter_map(|c| IriBuf::new(c.input).ok().map(|b| (c.label, b)))
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
