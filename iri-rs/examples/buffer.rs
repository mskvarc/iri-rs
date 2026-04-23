use iri_rs::IriBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut iri = IriBuf::new("https://www.rust-lang.org")?;

    iri.set_authority(Some("www.rust-lang.org:40"))?;
    iri.set_path("/foo/bar")?;
    iri.set_query(Some("query"))?;
    iri.set_fragment(Some("fragment"))?;

    assert_eq!(iri, "https://www.rust-lang.org:40/foo/bar?query#fragment");
    Ok(())
}
