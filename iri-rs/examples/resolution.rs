use iri_rs::{Iri, IriRefBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_iri = Iri::parse("http://a/b/c/d;p?q")?;
    let mut iri_ref = IriRefBuf::new("g;x=1/../y")?;

    let resolved = iri_ref.resolved(&base_iri)?;
    assert_eq!(resolved, "http://a/b/c/y");

    iri_ref.resolve(&base_iri)?;
    assert_eq!(iri_ref, "http://a/b/c/y");
    Ok(())
}
