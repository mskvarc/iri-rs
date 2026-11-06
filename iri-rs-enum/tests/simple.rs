use iri_rs_core::Iri;
use iri_rs_enum::IriEnum;
use iri_rs_static::iri;

#[test]
fn try_from() {
    #[derive(IriEnum, PartialEq, Debug)]
    #[iri_prefix("schema" = "https://schema.org/")]
    pub enum Vocab {
        #[iri("schema:name")]
        Name,
        #[iri("schema:knows")]
        Knows,
    }

    assert_eq!(Vocab::try_from(&iri!("https://schema.org/name")), Ok(Vocab::Name));
    assert_eq!(Vocab::try_from(&iri!("https://schema.org/knows")), Ok(Vocab::Knows));
    assert_eq!(Vocab::try_from(&iri!("https://schema.org/other")), Err(()));
}

#[test]
fn try_from_with_parameter() {
    #[derive(IriEnum, PartialEq, Debug)]
    #[iri_prefix("schema" = "https://schema.org/")]
    pub enum Vocab {
        #[iri("schema:name")]
        Name,
        #[iri("schema:knows")]
        Knows,
        Other(OtherVocab),
    }

    #[derive(IriEnum, PartialEq, Debug)]
    #[iri_prefix("schema" = "https://schema.org/")]
    pub enum OtherVocab {
        #[iri("schema:Text")]
        Text,
    }

    assert_eq!(Vocab::try_from(&iri!("https://schema.org/name")), Ok(Vocab::Name));
    assert_eq!(Vocab::try_from(&iri!("https://schema.org/knows")), Ok(Vocab::Knows));
    assert_eq!(Vocab::try_from(&iri!("https://schema.org/Text")), Ok(Vocab::Other(OtherVocab::Text)));
    assert_eq!(Vocab::try_from(&iri!("https://schema.org/other")), Err(()));
}

#[test]
fn round_trip_into_iri() {
    #[derive(IriEnum, PartialEq, Debug)]
    pub enum Vocab {
        #[iri("https://schema.org/name")]
        Name,
    }

    let back: Iri<&'static str> = (&Vocab::Name).into();
    assert_eq!(back.as_str(), "https://schema.org/name");
}

/// A variant matches by `==`, Simple String Comparison: only its declared
/// spelling matches, and an equivalent spelling is another IRI.
#[test]
fn try_from_matches_only_the_declared_spelling() {
    #[derive(IriEnum, PartialEq, Debug)]
    #[iri_prefix("schema" = "https://schema.org/")]
    pub enum Vocab {
        #[iri("schema:name")]
        Name,
        #[iri("HTTPS://Example.org/%7euser")]
        User,
        Other(OtherVocab),
    }

    #[derive(IriEnum, PartialEq, Debug)]
    pub enum OtherVocab {
        #[iri("https://schema.org/Text")]
        Text,
    }

    for (spelling, expected) in [
        ("https://schema.org/name", Ok(Vocab::Name)),
        ("HTTPS://SCHEMA.org/name", Err(())),
        ("https://schema.org/./%6Eame", Err(())),
        ("https://example.org/~user", Err(())),
        ("HTTPS://Example.org/%7euser", Ok(Vocab::User)),
        ("https://schema.org/Text", Ok(Vocab::Other(OtherVocab::Text))),
        ("https://Schema.org/Text", Err(())),
        ("https://schema.org/Name", Err(())),
    ] {
        assert_eq!(Vocab::try_from(&Iri::new(spelling).unwrap()), expected, "`{spelling}`");
    }
}

/// Converting the normal form accepts every equivalent spelling of a variant
/// declared in normal form.
#[test]
fn try_from_the_normal_form_matches_equivalent_spellings() {
    #[derive(IriEnum, PartialEq, Debug)]
    pub enum Vocab {
        #[iri("https://schema.org/name")]
        Name,
    }

    for spelling in ["https://schema.org/name", "HTTPS://SCHEMA.org/name", "https://schema.org/./%6Eame"] {
        assert_eq!(Vocab::try_from(&Iri::new(spelling).unwrap().normalized()), Ok(Vocab::Name), "`{spelling}`");
    }
}
