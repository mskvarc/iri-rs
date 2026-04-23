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
    assert_eq!(
        Vocab::try_from(&iri!("https://schema.org/Text")),
        Ok(Vocab::Other(OtherVocab::Text))
    );
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
