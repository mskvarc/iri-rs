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

/// A variant matches every IRI `==` to its own, not just its spelling.
#[test]
fn try_from_matches_equal_spellings() {
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

    // Under `bytewise`, `==` compares bytes, and so does the lookup.
    let bytewise = Iri::new("HTTP://a").unwrap() != Iri::new("http://a").unwrap();
    for (spelling, expected, declared) in [
        ("https://schema.org/name", Ok(Vocab::Name), true),
        ("HTTPS://SCHEMA.org/name", Ok(Vocab::Name), false),
        ("https://schema.org/./%6Eame", Ok(Vocab::Name), false),
        ("https://example.org/~user", Ok(Vocab::User), false),
        ("HTTPS://Example.org/%7euser", Ok(Vocab::User), true),
        ("https://schema.org/Text", Ok(Vocab::Other(OtherVocab::Text)), true),
        ("https://Schema.org/Text", Ok(Vocab::Other(OtherVocab::Text)), false),
        ("https://schema.org/Name", Err(()), false),
    ] {
        let expected = if bytewise && !declared { Err(()) } else { expected };
        assert_eq!(Vocab::try_from(&Iri::new(spelling).unwrap()), expected, "`{spelling}`");
    }
}
