use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs::{Iri, IriBuf, IriRef, IriRefBuf, Uri, UriBuf, UriRef, UriRefBuf};
use std::hint::black_box;

mod corpus;
use corpus::{INVALID, IRI_CORPUS, URI_CORPUS};

fn bench_parse(c: &mut Criterion) {
    let mut g = c.benchmark_group("facade_parse/iri_borrowed");
    for case in IRI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input, |b, s| {
            b.iter(|| Iri::new(black_box(s)).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("facade_parse/iri_owned");
    for case in IRI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input, |b, s| {
            b.iter(|| IriBuf::new(black_box(s.to_string())).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("facade_parse/iri_ref_borrowed");
    for case in IRI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input, |b, s| {
            b.iter(|| IriRef::new(black_box(s)).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("facade_parse/iri_ref_owned");
    for case in IRI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input, |b, s| {
            b.iter(|| IriRefBuf::new(black_box(s.to_string())).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("facade_parse/uri_borrowed");
    for case in URI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input.as_bytes(), |b, bytes| {
            b.iter(|| Uri::new(black_box(bytes)).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("facade_parse/uri_owned");
    for case in URI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input.as_bytes(), |b, bytes| {
            b.iter(|| UriBuf::new(black_box(bytes.to_vec())).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("facade_parse/uri_ref_borrowed");
    for case in URI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input.as_bytes(), |b, bytes| {
            b.iter(|| UriRef::new(black_box(bytes)).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("facade_parse/uri_ref_owned");
    for case in URI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input.as_bytes(), |b, bytes| {
            b.iter(|| UriRefBuf::new(black_box(bytes.to_vec())).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("facade_parse/reject_invalid");
    for case in INVALID {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input, |b, s| {
            b.iter(|| IriRef::new(black_box(s)).err());
        });
    }
    g.finish();
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
