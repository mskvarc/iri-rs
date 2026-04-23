use iri_rs::{InvalidIri, Iri};

fn main() -> Result<(), InvalidIri<&'static str>> {
    let iri = Iri::parse("https://www.rust-lang.org/foo/bar?query#frag")?;

    println!("scheme: {}", iri.scheme());
    println!("authority: {}", iri.authority().unwrap());
    println!("path: {}", iri.path());
    println!("query: {}", iri.query().unwrap());
    println!("fragment: {}", iri.fragment().unwrap());

    Ok(())
}
