# iri-rs-core

[![Crate](https://img.shields.io/crates/v/iri-rs-core.svg?style=flat-square)](https://crates.io/crates/iri-rs-core)
[![Docs](https://img.shields.io/docsrs/iri-rs-core?style=flat-square)](https://docs.rs/iri-rs-core)
[![MSRV](https://img.shields.io/crates/msrv/iri-rs-core?style=flat-square)](https://crates.io/crates/iri-rs-core)
[![License](https://img.shields.io/crates/l/iri-rs-core.svg?style=flat-square)](#license)

Core types and parser for the [`iri-rs`](../iri-rs/) workspace. Fork of [`iref`](https://crates.io/crates/iref) by [Timothée Haudebourg](https://github.com/timothee-haudebourg/iref). URI and IRI parsing, component access, in-place mutation, reference resolution, normalization, equivalence.

```text
  foo://user@example.com:8042/over/there?name=ferret#nose
  └┬┘   └──────────┬────────┘└────┬────┘└────┬─────┘└┬─┘
   │               │              │          │       │
 scheme        authority          path      query  fragment
```

Most users should depend on the facade crate [`iri-rs`](../iri-rs/) instead. Depend on `iri-rs-core` directly only if you want none of the optional features from the facade — or if you're building another crate on top (the `iri-rs-static` and `iri-rs-enum` proc-macros emit paths against `::iri_rs_core`, so downstream crates using them must have this crate as a direct dependency).

```rust
use iri_rs_core::{Iri, IriBuf};

let iri = Iri::parse("https://www.rust-lang.org/foo?q#f")?;
assert_eq!(iri.scheme(), "https");

let mut buf = IriBuf::new("https://rust-lang.org")?;
buf.set_path("/foo")?;
assert_eq!(buf, "https://rust-lang.org/foo");
```

---

## Install

```sh
cargo add iri-rs-core
```

## Feature flags

| Flag        | Default | Enables                                                            |
| ----------- | :-----: | ------------------------------------------------------------------ |
| `serde`     |         | `Serialize` / `Deserialize` for borrowed and owned types           |
| `bytewise`  |         | Compare, hash and order byte-wise instead of RFC 3987 normalization |

### `bytewise` changes comparison, not just hashing

`bytewise` swaps the `PartialEq`, `Eq`, `Hash`, `PartialOrd` and `Ord` impls of `Iri`, `IriRef`, `Uri` and `UriRef` for byte-wise ones. Without it, those impls compare and hash the RFC 3987 normalized form, so `http://example.com/%7Ea` and `http://example.com/~a` are equal and hash alike. With it, they are two different IRIs. Byte-wise is considerably cheaper, and it is the right choice when the IRIs you hold are already in the form you want to compare, but it is a semantic change and not only a speed one.

Cargo unifies features across a dependency graph, which makes this a global switch: one dependency enabling `bytewise` changes IRI equality for every crate in the build, including crates that never asked for it. Enable it in an application, or in a library whose whole graph you control, rather than in a library published for others to combine.

## What's inside

- `Iri` / `IriBuf`, `IriRef` / `IriRefBuf` — IRIs and IRI references (RFC 3987).
- `Uri` / `UriBuf`, `UriRef` / `UriRefBuf` — URIs and URI references (RFC 3986).
- `Positions` — pre-computed component ends. Used by `from_raw_parts` for const construction in [`iri-rs-static`](../iri-rs-static/) and [`iri-rs-enum`](../iri-rs-enum/).
- Strict RFC 3986 §5 reference resolution with [Errata 4547](https://www.rfc-editor.org/errata/eid4547).
- RFC 3986/3987 §6.2.2 normalization: `normalized()` returns the normal form that equality and hashing compare (protocol-agnostic, percent-decoding-aware); `normalized_for_scheme()` adds the §6.2.3 `http`/`https` defaults.

## Performance notes

- `simdutf8` for fast UTF-8 validation.
- `memchr` for `%` and delimiter scans.
- SWAR / byte-level fast paths on `new`, `eq`, `ord`, `hash`, `len`.
- `smallvec` for small-path accumulation in resolution / normalization.

## Benchmarks

```sh
cargo bench -p iri-rs-core
```

Covers `parse`, `accessors`, `resolve`, `normalize`, `mutate`, `normalize_eq`, `validate`. Criterion output: `target/criterion/`.

## Workspace

Part of the [`iri-rs`](https://github.com/mskvarc/iri-rs) workspace. See the [root README](../README.md) for the overall picture, motivation, and fork attribution.

## MSRV

Rust 1.85 (edition 2024).

## License

Dual-licensed:

- [Apache-2.0](../LICENSE-APACHE.md)
- [MIT](../LICENSE-MIT.md)
