use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs_core::{Iri, IriRef, IriRefBuf};
use std::hint::black_box;

mod corpus;
use corpus::{RFC3986_ABNORMAL, RFC3986_BASE, RFC3986_NORMAL};

fn bench_resolved(c: &mut Criterion) {
    let base = Iri::new(RFC3986_BASE).unwrap();

    let mut g = c.benchmark_group("resolve/rfc3986_normal");
    for (r, _) in RFC3986_NORMAL {
        let rref = IriRef::new(r).unwrap();
        g.bench_with_input(BenchmarkId::from_parameter(r), &rref, |b, rref| {
            b.iter(|| black_box(*rref).resolved(black_box(base)));
        });
    }
    g.finish();

    let mut g = c.benchmark_group("resolve/rfc3986_abnormal");
    for (r, _) in RFC3986_ABNORMAL {
        let rref = IriRef::new(r).unwrap();
        g.bench_with_input(BenchmarkId::from_parameter(r), &rref, |b, rref| {
            b.iter(|| black_box(*rref).resolved(black_box(base)));
        });
    }
    g.finish();
}

fn bench_in_place(c: &mut Criterion) {
    let base = Iri::new(RFC3986_BASE).unwrap();

    // In-place `resolve` avoids an extra allocation vs `resolved` — the
    // benchmark clones a seed per iteration (via iter_batched_ref) so the
    // buffer always starts in the same state.
    let mut g = c.benchmark_group("resolve/in_place");
    for (r, _) in RFC3986_NORMAL.iter().chain(RFC3986_ABNORMAL.iter()) {
        let seed = IriRefBuf::new(r.to_string()).unwrap();
        g.bench_with_input(BenchmarkId::from_parameter(r), &seed, |b, seed| {
            b.iter_batched_ref(
                || seed.clone(),
                |buf| {
                    buf.resolve(black_box(base));
                },
                BatchSize::SmallInput,
            );
        });
    }
    g.finish();
}

fn bench_relative_to(c: &mut Criterion) {
    // Inverse direction: given a resolved IRI, compute the relative form
    // against the base. Exercises a different control path from `resolved`.
    let base = Iri::new(RFC3986_BASE).unwrap();

    let mut g = c.benchmark_group("relative_to/rfc3986");
    for (_, full) in RFC3986_NORMAL.iter().chain(RFC3986_ABNORMAL.iter()) {
        let Ok(target) = Iri::new(full) else { continue };
        g.bench_with_input(BenchmarkId::from_parameter(full), &target, |b, target| {
            b.iter(|| black_box(*target).relative_to(base));
        });
    }
    g.finish();
}

criterion_group!(benches, bench_resolved, bench_in_place, bench_relative_to);
criterion_main!(benches);
