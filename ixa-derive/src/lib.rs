extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

use syn::{
    parse::{Parse, ParseStream, Result},
    // punctuated::Punctuated,
    // Expr,
    // ExprParen,
    // ExprTuple,
    Ident,
    Token,
    Type,
};

#[proc_macro_derive(IxaEvent)]
pub fn derive_ixa_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl IxaEvent for #name {}
    };

    TokenStream::from(expanded)
}

/// Struct to parse `sorted_tag_value_impl!(tag_tuple = (...), value_tuple = (...));`
struct TagValueInput {
    tag_tuple: Vec<Type>,
    value_tuple: Vec<Type>,
}

impl Parse for TagValueInput {
    fn parse(input: ParseStream) -> Result<Self> {
        // Parse `tag_tuple = ( ... ),`
        let tag_key: Ident = input.parse()?;
        if tag_key != "tag_tuple" {
            return Err(input.error("expected `tag_tuple`"));
        }
        input.parse::<Token![=]>()?;

        let tag_tuple;
        syn::parenthesized!(tag_tuple in input);
        let tags = tag_tuple.parse_terminated(Type::parse, Token![,])?;
        input.parse::<Token![,]>()?;

        // Parse `value_tuple = ( ... )`
        let value_key: Ident = input.parse()?;
        if value_key != "value_tuple" {
            return Err(input.error("expected `value_tuple`"));
        }
        input.parse::<Token![=]>()?;

        let value_tuple;
        syn::parenthesized!(value_tuple in input);
        let values = value_tuple.parse_terminated(Type::parse, Token![,])?;

        Ok(TagValueInput {
            tag_tuple: tags.into_iter().collect(),
            value_tuple: values.into_iter().collect(),
        })
    }
}

#[proc_macro]
pub fn sorted_tag_value_impl(input: TokenStream) -> TokenStream {
    let TagValueInput {
        tag_tuple,
        value_tuple,
    } = parse_macro_input!(input as TagValueInput);

    let orig_tags: Vec<_> = tag_tuple.into_iter().collect();
    let orig_values: Vec<_> = value_tuple.into_iter().collect();

    assert_eq!(orig_tags.len(), orig_values.len());

    // Compute sorted tags and permutation
    let mut indexed_tags: Vec<_> = orig_tags.iter().enumerate().collect();
    indexed_tags.sort_by_key(|(_, ty)| quote!(#ty).to_string());

    let sorted_tags: Vec<_> = indexed_tags.iter().map(|(_, t)| (*t).clone()).collect();
    let reordered_value_types: Vec<_> = indexed_tags
        .iter()
        .map(|(i, _)| orig_values[*i].clone())
        .collect();

    // Generate binding names
    let t_bindings: Vec<_> = (0..orig_values.len())
        .map(|i| Ident::new(&format!("t{}", i), Span::call_site()))
        .collect();
    let s_bindings: Vec<_> = (0..orig_values.len())
        .map(|i| Ident::new(&format!("s{}", i), Span::call_site()))
        .collect();

    // to_sorted: map sorted index to original binding
    let to_sorted_exprs: Vec<_> = indexed_tags
        .iter()
        .map(|(i, _)| {
            let ident = &t_bindings[*i];
            quote!(#ident)
        })
        .collect();

    // from_sorted: map original index to binding in sorted
    let from_sorted_exprs: Vec<_> = orig_tags
        .iter()
        .map(|ty| {
            let pos = sorted_tags
                .iter()
                .position(|t| quote!(#t).to_string() == quote!(#ty).to_string())
                .unwrap();
            let ident = &s_bindings[pos];
            quote!(#ident)
        })
        .collect();

    let tag_type = quote! { ( #( #orig_tags ),* ) };
    let value_type = quote! { ( #( #orig_values ),* ) };
    let sorted_tag_type = quote! { ( #( #sorted_tags ),* ) };
    let reordered_value_type = quote! { ( #( #reordered_value_types ),* ) };

    let expanded = quote! {
        impl SortByTag<#tag_type> for #value_type {
            type SortedTag = #sorted_tag_type;
            type ReorderedValue = #reordered_value_type;

            fn reorder_by_tag(self) -> Self::ReorderedValue {
                let ( #( #t_bindings ),* ) = self;
                ( #( #to_sorted_exprs ),* )
            }

            fn unreorder_by_tag(sorted: Self::ReorderedValue) -> Self {
                let ( #( #s_bindings ),* ) = sorted;
                ( #( #from_sorted_exprs ),* )
            }
        }
    };

    TokenStream::from(expanded)
}
