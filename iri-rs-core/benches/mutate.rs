use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use iri_rs_core::IriRefBuf;
use std::hint::black_box;

const SEED: &str = "http://example.com/a/b/c?q=1#frag";

fn seed() -> IriRefBuf {
    IriRefBuf::new(SEED.to_string()).unwrap()
}

fn bench_setters(c: &mut Criterion) {
    c.bench_function("mutate/set_scheme", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_scheme(Some(black_box("https"))), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_authority_replace", |b| {
        b.iter_batched_ref(
            seed,
            |buf| buf.set_authority(Some(black_box("user@other.example:9000"))),
            BatchSize::SmallInput,
        );
    });

    c.bench_function("mutate/set_authority_clear", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_authority(None), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_path_longer", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_path(black_box("/x/y/z/w")), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_query_replace", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_query(Some(black_box("k=v&m=n"))), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_query_clear", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_query(None), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_fragment_replace", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_fragment(Some(black_box("section-42"))), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_fragment_clear", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_fragment(None), BatchSize::SmallInput);
    });
}

criterion_group!(benches, bench_setters);
criterion_main!(benches);
