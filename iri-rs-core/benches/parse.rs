use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs_core::{Iri, IriBuf, IriRef, IriRefBuf, Uri, UriBuf, UriRef, UriRefBuf};
use std::hint::black_box;

mod corpus;
use corpus::{INVALID, IRI_CORPUS, REF_CORPUS, URI_CORPUS};

fn bench_iri(c: &mut Criterion) {
    let mut g = c.benchmark_group("parse/iri_borrowed");
    for case in IRI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input, |b, s| {
            b.iter(|| Iri::new(black_box(s)).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("parse/iri_owned");
    for case in IRI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input, |b, s| {
            b.iter(|| IriBuf::new(black_box(s.to_string())).ok());
        });
    }
    g.finish();
}

fn bench_iri_ref(c: &mut Criterion) {
    let mut g = c.benchmark_group("parse/iri_ref_borrowed");
    for case in IRI_CORPUS.iter().chain(REF_CORPUS.iter()) {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input, |b, s| {
            b.iter(|| IriRef::new(black_box(s)).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("parse/iri_ref_owned");
    for case in IRI_CORPUS.iter().chain(REF_CORPUS.iter()) {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input, |b, s| {
            b.iter(|| IriRefBuf::new(black_box(s.to_string())).ok());
        });
    }
    g.finish();
}

fn bench_uri(c: &mut Criterion) {
    let mut g = c.benchmark_group("parse/uri_borrowed");
    for case in URI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input.as_bytes(), |b, bytes| {
            b.iter(|| Uri::new(black_box(bytes)).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("parse/uri_owned");
    for case in URI_CORPUS {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input.as_bytes(), |b, bytes| {
            b.iter(|| UriBuf::new(black_box(bytes.to_vec())).ok());
        });
    }
    g.finish();
}

fn bench_uri_ref(c: &mut Criterion) {
    let mut g = c.benchmark_group("parse/uri_ref_borrowed");
    for case in URI_CORPUS.iter().chain(REF_CORPUS.iter()) {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input.as_bytes(), |b, bytes| {
            b.iter(|| UriRef::new(black_box(bytes)).ok());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("parse/uri_ref_owned");
    for case in URI_CORPUS.iter().chain(REF_CORPUS.iter()) {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input.as_bytes(), |b, bytes| {
            b.iter(|| UriRefBuf::new(black_box(bytes.to_vec())).ok());
        });
    }
    g.finish();
}

fn bench_reject(c: &mut Criterion) {
    let mut g = c.benchmark_group("parse/reject_invalid_iri_ref");
    for case in INVALID {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input, |b, s| {
            b.iter(|| IriRef::new(black_box(s)).err());
        });
    }
    g.finish();

    let mut g = c.benchmark_group("parse/reject_invalid_uri_ref");
    for case in INVALID {
        g.bench_with_input(BenchmarkId::from_parameter(case.label), case.input.as_bytes(), |b, bytes| {
            b.iter(|| UriRef::new(black_box(bytes)).err());
        });
    }
    g.finish();
}

criterion_group!(benches, bench_iri, bench_iri_ref, bench_uri, bench_uri_ref, bench_reject);
criterion_main!(benches);
