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
- SIMD-accelerated UTF-8 validation via `simdutf8`, `memchr`-accelerated percent scan, SWAR/byte-level fast paths on `new` and `len`.
- `==`, `Hash` and `Ord` are RFC 3987 §5.3.1 simple string comparison, with `Borrow<str>`; `equivalent()` and `normalized()` give the §5.3.2 syntax-based rung.
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

`IriRef` / `IriRefBuf` cover absolute *and* relative references. A strict implementation of the [RFC 3986 §5 reference resolution algorithm](https://tools.ietf.org/html/rfc3986#section-5), with its §5.4 results (`../../../g` against `http://a/b/c/d;p?q` is `http://a/g`):

```rust
use iri_rs::{Iri, IriRefBuf};

let base = Iri::parse("http://a/b/c/d;p?q")?;
let mut r = IriRefBuf::new("g;x=1/../y")?;
assert_eq!(r.resolved(&base)?, "http://a/b/c/y");

r.resolve(&base)?;
assert_eq!(r, "http://a/b/c/y");
```

## Comparison

`==`, `Hash` and `Ord` on `Iri`, `IriRef`, `Uri` and `UriRef` are RFC 3987 §5.3.1 Simple String Comparison: two values are equal exactly when their `as_str()` are, and they sort as their strings do, byte by byte. `Borrow<str>` agrees with them, so a map or set keyed by these types can be looked up by `&str`. Comparing with a `str`, `&str` or `String` is the same byte comparison.

`equivalent()` is the §5.3.2 rung, syntax-based normalization (RFC 3986 §6.2.2). It compares the components as it reads them, and allocates only for a component that holds an escape, uppercase host letters or dot segments:

- **Case** — the scheme and host are case-insensitive: `HTTP://Example.COM/` ≡ `http://example.com/`. The user info, path, query and fragment keep their case.
- **Percent encoding** — escaped unreserved characters decode and hex digits are case-insensitive, in every component: `http://exa%6dple.org/%7e` ≡ `http://example.org/~`, `%2f` ≡ `%2F`.
- **Dot segments** — `a/../a/./b/../b/c` ≡ `a/b/c`. Escapes decode first, so `/a/%2E%2E/b` ≡ `/b`. Under a scheme or an authority they go as resolution removes them, so an IRI is equivalent to what it resolves to: `foo:a/../../b` ≡ `foo:/b`. A relative-path reference keeps a `..` with nothing to climb over, since it still climbs the base it is resolved against: `a/../../b` ≡ `../b`, not `b`.
- **Protocol-agnostic** — `http://example.org` and `http://example.org:80` are **not** equivalent. `equivalent()` knows nothing about scheme defaults.
- **Every `/` counts** — `/foo/bar` and `/foo/bar/` are **not** equivalent.

`normalized()` returns that normal form: `a.equivalent(&b)` holds exactly when `a.normalized() == b.normalized()`. To key a map or set by normalized identity, normalize each IRI once and key by the result. `normalized_for_scheme()` also applies the §6.2.3 `http`/`https` defaults: it drops `:80`/`:443` and turns an empty path into `/`, and drops an empty port (`a:`) for any scheme. Its result therefore need not be equivalent to the input.

```rust
use iri_rs::IriBuf;

let iri = IriBuf::new("HTTP://Example.COM:80/a/./b/../%7Ec?Q=1")?;
assert_eq!(iri.normalized().as_str(), "http://example.com:80/a/~c?Q=1");
assert_eq!(iri.normalized_for_scheme().as_str(), "http://example.com/a/~c?Q=1");

let plain = IriBuf::new("http://example.com:80/a/~c?Q=1")?;
assert!(iri != plain);
assert!(iri.equivalent(&plain));
```

### Which comparison to use

RFC 3987 §5.1 leaves the choice to the application, and different specifications choose differently, sometimes in the same program. Identity tokens with no protocol behind them must use simple string comparison: RDF (RDF 1.1 Concepts §3.2) and JSON-LD compare IRIs with `==`. A protocol that asks for normalized comparison, such as NGSI-LD (ETSI GS CIM 009 §5.5.1, which defers to RFC 3986 §6), calls `equivalent()`, or normalizes once with `normalized()` where values enter and compares the results with `==`. Nothing rewrites an IRI in place, so one that is passed on keeps the form it arrived in, as RFC 3987 §5.3.2.3 requires.

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
