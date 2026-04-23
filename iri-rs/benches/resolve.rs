use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs::{Iri, IriRef, IriRefBuf};
use std::hint::black_box;

mod corpus;
use corpus::{RFC3986_BASE, RFC3986_PAIRS};

fn bench(c: &mut Criterion) {
    let base = Iri::new(RFC3986_BASE).unwrap();

    let mut g = c.benchmark_group("facade_resolve/resolved");
    for (r, _) in RFC3986_PAIRS {
        let rref = IriRef::new(r).unwrap();
        g.bench_with_input(BenchmarkId::from_parameter(r), &rref, |b, rref| {
            b.iter(|| black_box(*rref).resolved(black_box(base)));
        });
    }
    g.finish();

    let mut g = c.benchmark_group("facade_resolve/in_place");
    for (r, _) in RFC3986_PAIRS {
        let seed = IriRefBuf::new(r.to_string()).unwrap();
        g.bench_with_input(BenchmarkId::from_parameter(r), &seed, |b, seed| {
            b.iter_batched_ref(|| seed.clone(), |buf| buf.resolve(black_box(base)), BatchSize::SmallInput);
        });
    }
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
