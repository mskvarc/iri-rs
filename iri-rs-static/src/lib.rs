//! Compile-time macros for building `'static` IRIs / URIs.
//!
//! Macros validate at macro-expansion time via [`iri_rs_core`] and emit
//! `const` expressions that construct the type from pre-computed positions,
//! skipping all runtime work.

use iri_rs_core::{IriBuf, IriRefBuf, UriBuf, UriRefBuf};
use proc_macro::TokenStream;
use quote::quote;

fn positions_tokens(p: iri_rs_core::Positions) -> proc_macro2::TokenStream {
    let s = p.scheme_end;
    let a = p.authority_end;
    let pe = p.path_end;
    let q = p.query_end;
    quote! {
        ::iri_rs_core::Positions { scheme_end: #s, authority_end: #a, path_end: #pe, query_end: #q }
    }
}

#[proc_macro]
pub fn uri(tokens: TokenStream) -> TokenStream {
    match syn::parse::<syn::LitStr>(tokens) {
        Ok(lit) => {
            let v = lit.value();
            match UriBuf::new(v.as_bytes().to_vec()) {
                Ok(uri) => {
                    let s = uri.as_str();
                    let p = positions_tokens(uri.positions());
                    quote! {
                        ::iri_rs_core::Uri::<&'static str>::from_raw_parts(#s, #p)
                    }
                    .into()
                }
                Err(_) => produce_error("invalid URI"),
            }
        }
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn uri_ref(tokens: TokenStream) -> TokenStream {
    match syn::parse::<syn::LitStr>(tokens) {
        Ok(lit) => {
            let v = lit.value();
            match UriRefBuf::new(v.as_bytes().to_vec()) {
                Ok(uri_ref) => {
                    let s = uri_ref.as_str();
                    let p = positions_tokens(uri_ref.positions());
                    quote! {
                        ::iri_rs_core::UriRef::<&'static str>::from_raw_parts(#s, #p)
                    }
                    .into()
                }
                Err(_) => produce_error("invalid URI reference"),
            }
        }
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn iri(tokens: TokenStream) -> TokenStream {
    match syn::parse::<syn::LitStr>(tokens) {
        Ok(lit) => match IriBuf::new(lit.value()) {
            Ok(iri) => {
                let s = iri.as_str();
                let p = positions_tokens(iri.positions());
                quote! {
                    ::iri_rs_core::Iri::<&'static str>::from_raw_parts(#s, #p)
                }
                .into()
            }
            Err(_) => produce_error("invalid IRI"),
        },
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn iri_ref(tokens: TokenStream) -> TokenStream {
    match syn::parse::<syn::LitStr>(tokens) {
        Ok(lit) => match IriRefBuf::new(lit.value()) {
            Ok(iri_ref) => {
                let s = iri_ref.as_str();
                let p = positions_tokens(iri_ref.positions());
                quote! {
                    ::iri_rs_core::IriRef::<&'static str>::from_raw_parts(#s, #p)
                }
                .into()
            }
            Err(_) => produce_error("invalid IRI reference"),
        },
        Err(e) => e.to_compile_error().into(),
    }
}

fn produce_error(msg: &str) -> TokenStream {
    format!("compile_error!(\"{}\")", msg).parse().unwrap()
}
