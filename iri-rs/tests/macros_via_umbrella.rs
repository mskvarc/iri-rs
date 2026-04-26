#![cfg(all(feature = "static", feature = "enum"))]

use iri_rs::{IriEnum, iri};

#[test]
fn static_macro_via_umbrella() {
    let i = iri!("https://example.com/foo");
    assert_eq!(i.as_str(), "https://example.com/foo");
}

#[derive(IriEnum, PartialEq, Debug)]
#[iri_prefix("schema" = "https://schema.org/")]
enum Vocab {
    #[iri("schema:name")]
    Name,
}

#[test]
fn derive_via_umbrella() {
    assert_eq!(Vocab::try_from(&iri!("https://schema.org/name")), Ok(Vocab::Name),);
}
