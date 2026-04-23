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

criterion_group!(benches, bench_resolved, bench_in_place);
criterion_main!(benches);
