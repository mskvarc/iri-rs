use iri_rs::{IriBuf, IriRefBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Foo {
    iri: IriBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct Bar {
    iri_ref: IriRefBuf,
}

fn main() {
    let foo: Foo = serde_json::from_str("{ \"iri\": \"https://example.org/foo\" }").unwrap();
    let bar: Bar = serde_json::from_str("{ \"iri_ref\": \"../bar\" }").unwrap();

    eprintln!("{:?}", foo);
    eprintln!("{:?}", bar);
}
