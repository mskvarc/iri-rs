use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iri_rs_core::{Iri, IriBuf, Uri, UriBuf};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    hint::black_box,
};

mod corpus;
use corpus::{IRI_CORPUS, URI_CORPUS};

fn parsed_iris() -> Vec<(&'static str, Iri<&'static str>)> {
    IRI_CORPUS
        .iter()
        .filter_map(|c| Iri::parse(c.input).ok().map(|i| (c.label, i)))
        .collect()
}

fn parsed_uris() -> Vec<(&'static str, Uri<&'static str>)> {
    URI_CORPUS
        .iter()
        .filter_map(|c| Uri::parse(c.input).ok().map(|u| (c.label, u)))
        .collect()
}

fn iri_bufs() -> Vec<(&'static str, IriBuf)> {
    IRI_CORPUS
        .iter()
        .filter_map(|c| IriBuf::new(c.input).ok().map(|b| (c.label, b)))
        .collect()
}

fn uri_bufs() -> Vec<(&'static str, UriBuf)> {
    URI_CORPUS
        .iter()
        .filter_map(|c| UriBuf::new(c.input.as_bytes().to_vec()).ok().map(|b| (c.label, b)))
        .collect()
}

fn bench_component_extract(c: &mut Criterion) {
    let iris = parsed_iris();
    let uris = parsed_uris();

    let mut g = c.benchmark_group("accessors/iri_fields_each");
    for (label, iri) in &iris {
        g.bench_with_input(BenchmarkId::from_parameter(label), iri, |b, iri| {
            b.iter(|| {
                let iri = black_box(*iri);
                let s = iri.scheme().as_bytes().len();
                let a = iri.authority().map(|x| x.as_bytes().len()).unwrap_or(0);
                let p = iri.path().as_bytes().len();
                let q = iri.query().map(|x| x.as_bytes().len()).unwrap_or(0);
                let f = iri.fragment().map(|x| x.as_bytes().len()).unwrap_or(0);
                s + a + p + q + f
            });
        });
    }
    g.finish();

    let mut g = c.benchmark_group("accessors/uri_fields_each");
    for (label, uri) in &uris {
        g.bench_with_input(BenchmarkId::from_parameter(label), uri, |b, uri| {
            b.iter(|| {
                let uri = black_box(*uri);
                let s = uri.scheme().as_bytes().len();
                let a = uri.authority().map(|x| x.as_bytes().len()).unwrap_or(0);
                let p = uri.path().as_bytes().len();
                let q = uri.query().map(|x| x.as_bytes().len()).unwrap_or(0);
                let f = uri.fragment().map(|x| x.as_bytes().len()).unwrap_or(0);
                s + a + p + q + f
            });
        });
    }
    g.finish();
}

fn bench_equality(c: &mut Criterion) {
    let iris = parsed_iris();

    let mut g = c.benchmark_group("eq/iri_same");
    for (label, iri) in &iris {
        g.bench_with_input(BenchmarkId::from_parameter(label), iri, |b, iri| {
            b.iter(|| black_box(*iri) == black_box(*iri));
        });
    }
    g.finish();

    let mut g = c.benchmark_group("eq/iri_mixed");
    for i in 0..iris.len() {
        let (label, a) = iris[i];
        let (_, b_iri) = iris[(i + 1) % iris.len()];
        g.bench_with_input(BenchmarkId::from_parameter(label), &(a, b_iri), |bn, (a, b_iri)| {
            bn.iter(|| black_box(*a) == black_box(*b_iri));
        });
    }
    g.finish();
}

fn bench_hash(c: &mut Criterion) {
    let bufs = iri_bufs();
    let ubufs = uri_bufs();

    let mut g = c.benchmark_group("hash/iri_buf");
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

    let mut g = c.benchmark_group("hash/uri_buf");
    for (label, buf) in &ubufs {
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

criterion_group!(benches, bench_component_extract, bench_equality, bench_hash);
criterion_main!(benches);
