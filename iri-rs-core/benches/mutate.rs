use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use iri_rs_core::{
    IriRefBuf,
    iri::{Authority, Fragment, Path, Query, Segment},
    uri::Scheme,
};
use std::hint::black_box;

const SEED: &str = "http://example.com/a/b/c?q=1#frag";

fn seed() -> IriRefBuf {
    IriRefBuf::new(SEED.to_string()).unwrap()
}

fn bench_setters(c: &mut Criterion) {
    let new_scheme = Scheme::new(b"https").unwrap();
    let new_authority = Authority::new("user@other.example:9000").unwrap();
    let new_path_buf = Path::new("/x/y/z/w").unwrap();
    let new_query = Query::new("k=v&m=n").unwrap();
    let new_fragment = Fragment::new("section-42").unwrap();

    c.bench_function("mutate/set_scheme", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_scheme(Some(black_box(new_scheme))), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_authority_replace", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_authority(Some(black_box(new_authority))), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_authority_clear", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_authority(None), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_path_longer", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_path(black_box(new_path_buf)), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_query_replace", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_query(Some(black_box(new_query))), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_query_clear", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_query(None), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_fragment_replace", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_fragment(Some(black_box(new_fragment))), BatchSize::SmallInput);
    });

    c.bench_function("mutate/set_fragment_clear", |b| {
        b.iter_batched_ref(seed, |buf| buf.set_fragment(None), BatchSize::SmallInput);
    });
}

fn bench_path_mut(c: &mut Criterion) {
    let seg = Segment::new("zzz").unwrap();

    c.bench_function("mutate/path_mut_push", |b| {
        b.iter_batched_ref(seed, |buf| buf.path_mut().push(black_box(seg)), BatchSize::SmallInput);
    });

    c.bench_function("mutate/path_mut_pop", |b| {
        b.iter_batched_ref(seed, |buf| buf.path_mut().pop(), BatchSize::SmallInput);
    });

    // One push + one pop: length-neutral cycle. Measures amortised cost of
    // the mutable path operations.
    c.bench_function("mutate/path_mut_push_pop_cycle", |b| {
        b.iter_batched_ref(
            seed,
            |buf| {
                let mut p = buf.path_mut();
                p.push(black_box(seg));
                p.pop();
            },
            BatchSize::SmallInput,
        );
    });

    // Symbolic push respects dot-segments.
    let dotdot = Segment::new("..").unwrap();
    c.bench_function("mutate/path_mut_symbolic_push_dotdot", |b| {
        b.iter_batched_ref(seed, |buf| buf.path_mut().symbolic_push(black_box(dotdot)), BatchSize::SmallInput);
    });
}

criterion_group!(benches, bench_setters, bench_path_mut);
criterion_main!(benches);
