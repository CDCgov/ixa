use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::Parse, Ident, Result, Token};

use crate::utilities::{resolved_person_property_path, TypeTuple};

fn reorder_indices(tag: &[Ident]) -> Vec<usize> {
    let mut sorted: Vec<_> = tag.iter().enumerate().collect();
    sorted.sort_by(|(_, a), (_, b)| a.to_string().cmp(&b.to_string()));
    sorted.iter().map(|(i, _)| *i).collect()
}

fn inverse_indices(indices: &[usize]) -> Vec<usize> {
    let mut inv = vec![0; indices.len()];
    for (i, &idx) in indices.iter().enumerate() {
        inv[idx] = i;
    }
    inv
}

pub fn reorder_fn(tag: &TypeTuple) -> proc_macro2::TokenStream {
    let person_property_path = resolved_person_property_path();
    let indices = reorder_indices(&tag.0);

    let vars: Vec<_> = (0..tag.0.len()).map(|i| format_ident!("v{}", i)).collect();
    let reordered = indices.iter().map(|&i| &vars[i]);

    // Types in tag order = Value
    let value_types_unsorted: Vec<_> = tag
        .0
        .iter()
        .map(|ident| quote! { <#ident as #person_property_path>::Value })
        .collect();

    // Types in sorted tag order = CanonicalValue
    let mut sorted_idents = tag.0.clone();
    sorted_idents.sort_by_key(|a| a.to_string());
    let value_types_sorted: Vec<_> = sorted_idents
        .iter()
        .map(|ident| quote! { <#ident as #person_property_path>::Value })
        .collect();

    quote! {
        fn reorder_by_tag((#( #vars ),*): (#( #value_types_unsorted ),*)) -> (#( #value_types_sorted ),*) {
            ( #( #reordered ),* )
        }
    }
}

pub fn unreorder_fn(tag: &TypeTuple) -> proc_macro2::TokenStream {
    let person_property_path = resolved_person_property_path();
    let indices = reorder_indices(&tag.0);
    let inverse = inverse_indices(&indices);

    let vars: Vec<_> = (0..tag.0.len()).map(|i| format_ident!("v{}", i)).collect();
    let unreordered = inverse.iter().map(|&i| &vars[i]);

    // Types in tag order = Value
    let value_types_unsorted: Vec<_> = tag
        .0
        .iter()
        .map(|ident| quote! { <#ident as #person_property_path>::Value })
        .collect();

    // Types in sorted tag order = CanonicalValue
    let mut sorted_idents = tag.0.clone();
    sorted_idents.sort_by_key(|a| a.to_string());
    let value_types_sorted: Vec<_> = sorted_idents
        .iter()
        .map(|ident| quote! { <#ident as #person_property_path>::Value })
        .collect();

    quote! {
        fn unreorder_by_tag((#( #vars ),*): (#( #value_types_sorted ),*)) -> (#( #value_types_unsorted ),*) {
            ( #( #unreordered ),* )
        }
    }
}

/// Expands to an `impl $struct_name` block that defines make_canonical and make_uncanonical methods.
pub fn impl_reoder_fns(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as ImplMakeCanonicalInput);
    let ImplMakeCanonicalInput { struct_name, tag } = input;

    let reorder = reorder_fn(&tag);
    let unreorder = unreorder_fn(&tag);

    let output = quote! {
        impl #struct_name {
            #reorder
            #unreorder
        }
    };
    output.into()
}

struct ImplMakeCanonicalInput {
    struct_name: Ident,
    tag: TypeTuple,
}

impl Parse for ImplMakeCanonicalInput {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let struct_name: Ident = input.parse()?;
        let _: Token![,] = input.parse()?;
        let tag: TypeTuple = input.parse()?;
        Ok(Self { struct_name, tag })
    }
}
