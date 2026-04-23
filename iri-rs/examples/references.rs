use iri_rs::IriRefBuf;

fn main() {
    let mut iri_ref = IriRefBuf::default();

    iri_ref.set_scheme(Some("https")).unwrap();
    iri_ref.set_authority(Some("example.com")).unwrap();
    let _iri = iri_ref.as_iri().unwrap();
}
