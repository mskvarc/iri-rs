use iri_rs_core::Uri;
use iri_rs_static::uri;

#[test]
fn const_uri() {
    const U: Uri<&'static str> = uri!("https://example.com/foo");
    assert_eq!(U.as_str(), "https://example.com/foo");
}
