//! Compile-time macros for building `'static` IRIs / URIs.
//!
//! Macros validate at macro-expansion time via [`iri_rs_core`] and emit
//! `const` expressions that construct the type from pre-computed positions,
//! skipping all runtime work.

use iri_rs_core::{IriBuf, IriRefBuf, UriBuf, UriRefBuf};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::{format_ident, quote};

fn core_path() -> TokenStream2 {
    match crate_name("iri-rs") {
        Ok(FoundCrate::Itself) => quote!(::iri_rs::__private),
        Ok(FoundCrate::Name(n)) => {
            let id = format_ident!("{}", n);
            quote!(::#id::__private)
        }
        Err(_) => match crate_name("iri-rs-core")
            .expect("expected `iri-rs` or `iri-rs-core` in dependencies")
        {
            FoundCrate::Itself => quote!(crate),
            FoundCrate::Name(n) => {
                let id = format_ident!("{}", n);
                quote!(::#id)
            }
        },
    }
}

fn positions_tokens(core: &TokenStream2, p: iri_rs_core::Positions) -> TokenStream2 {
    let s = p.scheme_end;
    let a = p.authority_end;
    let pe = p.path_end;
    let q = p.query_end;
    quote! {
        #core::Positions { scheme_end: #s, authority_end: #a, path_end: #pe, query_end: #q }
    }
}

#[proc_macro]
pub fn uri(tokens: TokenStream) -> TokenStream {
    match syn::parse::<syn::LitStr>(tokens) {
        Ok(lit) => {
            let v = lit.value();
            match UriBuf::new(v.as_bytes().to_vec()) {
                Ok(uri) => {
                    let core = core_path();
                    let s = uri.as_str();
                    let p = positions_tokens(&core, uri.positions());
                    quote! {
                        #core::Uri::<&'static str>::from_raw_parts(#s, #p)
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
                    let core = core_path();
                    let s = uri_ref.as_str();
                    let p = positions_tokens(&core, uri_ref.positions());
                    quote! {
                        #core::UriRef::<&'static str>::from_raw_parts(#s, #p)
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
                let core = core_path();
                let s = iri.as_str();
                let p = positions_tokens(&core, iri.positions());
                quote! {
                    #core::Iri::<&'static str>::from_raw_parts(#s, #p)
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
                let core = core_path();
                let s = iri_ref.as_str();
                let p = positions_tokens(&core, iri_ref.positions());
                quote! {
                    #core::IriRef::<&'static str>::from_raw_parts(#s, #p)
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
