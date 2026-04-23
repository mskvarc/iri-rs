# iri-rs-static

[![Crate](https://img.shields.io/crates/v/iri-rs-static.svg?style=flat-square)](https://crates.io/crates/iri-rs-static)
[![Docs](https://img.shields.io/docsrs/iri-rs-static?style=flat-square)](https://docs.rs/iri-rs-static)
[![MSRV](https://img.shields.io/crates/msrv/iri-rs-static?style=flat-square)](https://crates.io/crates/iri-rs-static)
[![License](https://img.shields.io/crates/l/iri-rs-static.svg?style=flat-square)](#license)

Compile-time macros for building `'static` URIs and IRIs for the [`iri-rs`](../iri-rs/) workspace. Fork of [`static-iref`](https://crates.io/crates/static-iref) by [Timothée Haudebourg](https://github.com/timothee-haudebourg/iref).

```rust
use iri_rs_core::Iri;
use iri_rs_static::iri;

const HOME: Iri<&'static str> = iri!("https://www.rust-lang.org/");
```

Invalid literals fail at compile time:

```rust,compile_fail
let _ = iri_rs_static::iri!("not a valid iri");
```

Each macro validates its input with the real parser from [`iri-rs-core`](../iri-rs-core/), then emits a `const` expression that constructs the type from pre-computed component positions — no runtime parsing, no allocation.

---

## Install

Enable the `static` feature on the facade crate (recommended):

```sh
cargo add iri-rs --features static
```

or depend on this crate directly:

```sh
cargo add iri-rs-core iri-rs-static
```

Downstream crates must have `iri-rs-core` as a direct dependency — the generated code references `::iri_rs_core::*`.

## Macros

| Macro       | Produces                              |
| ----------- | ------------------------------------- |
| `uri!`      | `Uri<&'static str>`                   |
| `uri_ref!`  | `UriRef<&'static str>`                |
| `iri!`      | `Iri<&'static str>`                   |
| `iri_ref!`  | `IriRef<&'static str>`                |

```rust
use iri_rs_core::{Iri, IriRef, Uri, UriRef};
use iri_rs_static::{iri, iri_ref, uri, uri_ref};

const A: Uri<&'static str> = uri!("https://example.org/");
const B: UriRef<&'static str> = uri_ref!("/foo/bar");
const C: Iri<&'static str> = iri!("https://café.example/");
const D: IriRef<&'static str> = iri_ref!("../sibling");
```

## Workspace

Part of the [`iri-rs`](https://github.com/mskvarc/iri-rs) workspace. See the [root README](../README.md) for the overall picture, motivation, and fork attribution.

## MSRV

Rust 1.85 (edition 2024).

## License

Dual-licensed:

- [Apache-2.0](../LICENSE-APACHE.md)
- [MIT](../LICENSE-MIT.md)
