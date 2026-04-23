# iri-rs

[![Crate](https://img.shields.io/crates/v/iri-rs.svg?style=flat-square)](https://crates.io/crates/iri-rs)
[![Docs](https://img.shields.io/docsrs/iri-rs?style=flat-square)](https://docs.rs/iri-rs)
[![MSRV](https://img.shields.io/crates/msrv/iri-rs?style=flat-square)](https://crates.io/crates/iri-rs)
[![License](https://img.shields.io/crates/l/iri-rs.svg?style=flat-square)](#license)

An allocation-conscious Rust implementation of URIs and IRIs ([RFC 3986][uri-rfc] / [RFC 3987][iri-rfc]) — parse, access components, mutate in-place, resolve references, normalize, compare.

A URI/IRI is a sequence of characters split into distinguishable components:

```text
  foo://user@example.com:8042/over/there?name=ferret#nose
  └┬┘   └──────────┬────────┘└────┬────┘└────┬─────┘└┬─┘
   │               │              │          │       │
 scheme        authority          path      query  fragment
```

This crate gives you typed borrowed and owned views over each of them.

```rust
use iri_rs::Iri;

let iri = Iri::parse("https://www.rust-lang.org/foo/bar?query#frag")?;
assert_eq!(iri.scheme(), "https");
assert_eq!(iri.authority(), Some("www.rust-lang.org"));
assert_eq!(iri.path(), "/foo/bar");
assert_eq!(iri.query(), Some("query"));
assert_eq!(iri.fragment(), Some("frag"));
```

Mutate in place, no per-component allocations:

```rust
use iri_rs::IriBuf;

let mut iri = IriBuf::new("https://rust-lang.org")?;
iri.set_authority(Some("rust-lang.org:40"))?;
iri.set_path("/foo/bar")?;
iri.set_query(Some("q"))?;
assert_eq!(iri, "https://rust-lang.org:40/foo/bar?q");
```

---

## Why this fork

Fork of [`iref`](https://crates.io/crates/iref), [`static-iref`](https://crates.io/crates/static-iref), and [`iref-enum`](https://crates.io/crates/iref-enum) by [Timothée Haudebourg](https://github.com/timothee-haudebourg/iref). Public API and RFC behavior largely unchanged; this fork adds:

- Workspace split into [`iri-rs-core`](iri-rs-core/), [`iri-rs-static`](iri-rs-static/), [`iri-rs-enum`](iri-rs-enum/) under the [`iri-rs`](iri-rs/) facade.
- Reworked `IriEnum` derive ([`iri-rs-enum`](iri-rs-enum/)) — compile-time prefix resolution, `const`-friendly output via `from_raw_parts`.
- Reworked compile-time macros (`iri!`, `uri!`, `iri_ref!`, `uri_ref!`) — validate at macro expansion, emit `const`-friendly `from_raw_parts` with pre-computed component positions.
- SIMD-accelerated UTF-8 validation via `simdutf8`, `memchr`-accelerated percent scan, SWAR/byte-level fast paths on `new`, `eq`, `ord`, `hash`, `len`.
- Opt-in `fast-hash` feature for hashing without re-parsing.
- Criterion bench suite (`parse`, `accessors`, `resolve`, `normalize`, `mutate`, `normalize_eq`, `validate`).
- Rust 2024 edition, MSRV 1.85.
- Renamed crates: `iref` → `iri-rs`, `static-iref` → `iri-rs-static`, `iref-enum` → `iri-rs-enum`.

Credit and history preserved — see [Attribution](#attribution).

## Install

```sh
cargo add iri-rs
```

## Feature flags

| Flag        | Default | Enables                                                                   |
| ----------- | :-----: | ------------------------------------------------------------------------- |
| `static`    |         | Compile-time `iri!`, `uri!`, `iri_ref!`, `uri_ref!` macros                |
| `enum`      |         | `#[derive(IriEnum)]` for vocabulary enums                                 |
| `serde`     |         | `Serialize` / `Deserialize` for borrowed and owned types                  |
| `fast-hash` |         | Skip re-parse on `Hash` — trades stricter invariants for speed            |

## Compile-time IRIs

With `static`, IRIs are validated at macro expansion and emitted as `const` expressions — zero runtime parsing, zero allocation:

```rust
use iri_rs::{Iri, iri};

const HOME: Iri<&'static str> = iri!("https://www.rust-lang.org/");
```

Invalid literals fail to compile. Same for `uri!`, `iri_ref!`, `uri_ref!`.

## Vocabulary enums

With `enum`, map a known vocabulary to a plain enum — cheap storage and comparison, compile-time prefix resolution:

```rust
use iri_rs::{iri, IriEnum};

#[derive(IriEnum, PartialEq, Debug)]
#[iri_prefix("schema" = "https://schema.org/")]
pub enum Vocab {
    #[iri("schema:name")] Name,
    #[iri("schema:knows")] Knows,
}

let term = Vocab::try_from(&iri!("https://schema.org/name")).unwrap();
assert_eq!(term, Vocab::Name);
```

## References and resolution

`IriRef` / `IriRefBuf` cover absolute *and* relative references. A strict implementation of the [RFC 3986 §5 reference resolution algorithm](https://tools.ietf.org/html/rfc3986#section-5) with [Errata 4547](https://www.rfc-editor.org/errata/eid4547):

```rust
use iri_rs::{Iri, IriRefBuf};

let base = Iri::parse("http://a/b/c/d;p?q")?;
let mut r = IriRefBuf::new("g;x=1/../y")?;
assert_eq!(r.resolved(&base)?, "http://a/b/c/y");

r.resolve(&base)?;
assert_eq!(r, "http://a/b/c/y");
```

## Equivalence

Equality, ordering and hashing normalize:

- **Dot segments** — `a/../a/./b/../b/c` ≡ `a/b/c`.
- **Percent encoding** — `http://example.org` ≡ `http://exa%6dple.org` (via [`pct`](https://crates.io/crates/pct)).
- **Protocol-agnostic** — `http://example.org` and `http://example.org:80` are **not** equal. This crate knows nothing about scheme defaults.
- **Every `/` counts** — `/foo/bar` and `/foo/bar/` are **not** equal.

Two values whose `as_str()` differs can still hash equal — keep that in mind when used as map keys.

## Examples

```sh
cargo run --example serde --features serde
```

## Benchmarks

```sh
cargo bench -p iri-rs-core
```

Criterion output: `target/criterion/`.

## MSRV

Rust 1.85 (edition 2024).

## Workspace layout

- [`iri-rs`](iri-rs/) — facade crate. Re-exports everything behind feature flags. Start here.
- [`iri-rs-core`](iri-rs-core/) — types, parser, resolver, normalizer.
- [`iri-rs-static`](iri-rs-static/) — `iri!` / `uri!` / `iri_ref!` / `uri_ref!` compile-time macros.
- [`iri-rs-enum`](iri-rs-enum/) — `#[derive(IriEnum)]` for vocabulary enums.

## Attribution

Original crates: [`iref`](https://crates.io/crates/iref), [`static-iref`](https://crates.io/crates/static-iref), and [`iref-enum`](https://crates.io/crates/iref-enum) by [Timothée Haudebourg](https://github.com/timothee-haudebourg/iref). Upstream commits are preserved in this repo's history under their original authorship. This fork is a workspace reorganization and a layer of performance work on top of the original design.

## License

Dual-licensed, same as upstream. Pick whichever fits:

- [Apache-2.0](LICENSE-APACHE.md)
- [MIT](LICENSE-MIT.md)

[uri-rfc]: https://tools.ietf.org/html/rfc3986
[iri-rfc]: https://tools.ietf.org/html/rfc3987
