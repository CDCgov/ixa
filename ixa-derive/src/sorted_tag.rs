use crate::utilities::{resolved_person_property_path, TypeTuple};
use proc_macro::TokenStream;
use quote::quote;

/// Returns a new tuple with identifiers sorted alphabetically
pub fn sorted_tag(input: TokenStream) -> TokenStream {
    let TypeTuple(mut idents) = syn::parse_macro_input!(input as TypeTuple);
    idents.sort_by_key(|a| a.to_string());
    let output = quote! { ( #( #idents ),* ) };
    output.into()
}

/// Returns a tuple of `<T::Value>` in the order of the sorted tag (names of `T` in alphabetical order).
pub fn sorted_value_type(input: TokenStream) -> TokenStream {
    let person_property_path = resolved_person_property_path();
    let TypeTuple(mut idents) = syn::parse_macro_input!(input as TypeTuple);
    idents.sort_by_key(|a| a.to_string());
    let values = idents
        .iter()
        .map(|id| quote! { <#id as #person_property_path>::Value });
    let output = quote! { ( #( #values ),* ) };
    output.into()
}
