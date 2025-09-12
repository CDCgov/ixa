use proc_macro2::Ident;
use proc_macro_crate::{crate_name, FoundCrate};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Path, Token};

/// Returns a path to the `ixa::people::PersonProperty` or `crate::people::PersonProperty`
pub(crate) fn resolved_person_property_path() -> Path {
    match crate_name("ixa") {
        Ok(FoundCrate::Itself) => syn::parse_quote!(crate::people::PersonProperty),
        Ok(FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
            syn::parse_quote!(#ident::people::PersonProperty)
        }
        Err(e) => panic!("Failed to find crate `ixa`: {e}"),
    }
}

/// A utility struct that parses an input of the form: (A, B, C)
#[derive(Clone, Debug)]
pub(crate) struct TypeTuple(pub Vec<Ident>);

impl Parse for TypeTuple {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        let inner: Punctuated<Ident, Token![,]> =
            content.parse_terminated(Ident::parse, Token![,])?;
        Ok(TypeTuple(inner.into_iter().collect()))
    }
}
