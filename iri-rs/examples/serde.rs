use iri_rs::{IriBuf, IriRefBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Resource {
    iri: IriBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct Link {
    iri_ref: IriRefBuf,
}

fn main() {
    let resource: Resource = serde_json::from_str("{ \"iri\": \"https://example.org/foo\" }").unwrap();
    let link: Link = serde_json::from_str("{ \"iri_ref\": \"../bar\" }").unwrap();

    eprintln!("{:?}", resource);
    eprintln!("{:?}", link);
}
