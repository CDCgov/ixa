extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

use syn::{
    parse::{Parse, ParseStream, Result},
    punctuated::Punctuated,
    Expr,
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

    // Resolve the path to the `ixa` crate
    let ixa_path = match crate_name("ixa") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Err(_) => quote!(::ixa), // fallback if discovery fails
    };

    let tag_type = quote! { ( #( #orig_tags ),* ) };
    let value_type = quote! { ( #( #orig_values ),* ) };
    let sorted_tag_type = quote! { ( #( #sorted_tags ),* ) };
    let reordered_value_type = quote! { ( #( #reordered_value_types ),* ) };

    let expanded = quote! {
        impl #ixa_path::people::SortByTag<#tag_type> for #value_type {
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

/// `static_sorted_tuples!{ ... }` supports two input "modes":
/// 1) Values mode:
///    tag_tuple = ( ... );
///    value_tuple = ( ... );
/// 2) Types/associated-types mode:
///    type SortedTag = ( ... );
///    type ReorderedValue = ( ... );
enum StaticInput {
    Values {
        tag_ident: Ident,
        value_ident: Ident,
        tag_elems: Vec<Expr>,
        value_elems: Vec<Expr>,
    },
    Types {
        tag_ident: Ident,
        value_ident: Ident,
        tag_types: Vec<Type>,
        value_types: Vec<Type>,
    },
}

impl Parse for StaticInput {
    fn parse(input: ParseStream) -> Result<Self> {
        // Peek to decide which grammar we're parsing.
        if input.peek(Token![type]) {
            // Parse: `type SortedTag = ( ... );`
            input.parse::<Token![type]>()?;
            let first_ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let ts1;
            syn::parenthesized!(ts1 in input);
            let sorted_tag_types: Punctuated<Type, Token![,]> =
                ts1.parse_terminated(Type::parse, Token![,])?;
            // optional `;`
            let _ = input.parse::<Token![;]>();

            // Parse: `type ReorderedValue = ( ... );`
            input.parse::<Token![type]>()?;
            let second_ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let ts2;
            syn::parenthesized!(ts2 in input);
            let reordered_value_types: Punctuated<Type, Token![,]> =
                ts2.parse_terminated(Type::parse, Token![,])?;
            // optional `;`
            let _ = input.parse::<Token![;]>();

            Ok(StaticInput::Types {
                tag_ident: first_ident,
                value_ident: second_ident,
                tag_types: sorted_tag_types.into_iter().collect(),
                value_types: reordered_value_types.into_iter().collect(),
            })
        } else {
            // Parse values mode:
            // `tag_tuple = ( ... );`
            let tag_key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let tts;
            syn::parenthesized!(tts in input);
            let tag_elems: Punctuated<Expr, Token![,]> =
                tts.parse_terminated(Expr::parse, Token![,])?;
            let _ = input.parse::<Token![;]>();

            // `value_tuple = ( ... );`
            let value_key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let vts;
            syn::parenthesized!(vts in input);
            let value_elems: Punctuated<Expr, Token![,]> =
                vts.parse_terminated(Expr::parse, Token![,])?;
            let _ = input.parse::<Token![;]>();

            Ok(StaticInput::Values {
                tag_ident: tag_key,
                value_ident: value_key,
                tag_elems: tag_elems.into_iter().collect(),
                value_elems: value_elems.into_iter().collect(),
            })
        }
    }
}

#[proc_macro]
pub fn static_sorted_tuples(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as StaticInput);

    match parsed {
        StaticInput::Values {
            tag_ident,
            value_ident,
            tag_elems,
            value_elems,
        } => {
            assert_eq!(
                tag_elems.len(),
                value_elems.len(),
                "tag/value tuple lengths must match"
            );

            // Build stable permutation by sorting the **stringified** tag elements.
            let mut idx: Vec<(usize, String)> = tag_elems
                .iter()
                .enumerate()
                .map(|(i, t)| (i, quote!(#t).to_string()))
                .collect();
            idx.sort_by(|a, b| a.1.cmp(&b.1)); // stable

            let sorted_tags = idx.iter().map(|(i, _)| tag_elems[*i].clone());
            let reordered_vals = idx.iter().map(|(i, _)| value_elems[*i].clone());

            let expanded = quote! {
                #tag_ident = ( #( #sorted_tags ),* );
                #value_ident = ( #( #reordered_vals ),* );
            };
            TokenStream::from(expanded)
        }

        StaticInput::Types {
            tag_ident,
            value_ident,
            tag_types,
            value_types,
        } => {
            assert_eq!(
                tag_types.len(),
                value_types.len(),
                "SortedTag/ReorderedValue tuple lengths must match"
            );

            let mut idx: Vec<(usize, String)> = tag_types
                .iter()
                .enumerate()
                .map(|(i, t)| (i, quote!(#t).to_string()))
                .collect();
            idx.sort_by(|a, b| a.1.cmp(&b.1)); // stable

            let sorted_tag_types = idx.iter().map(|(i, _)| tag_types[*i].clone());
            let reordered_value_types = idx.iter().map(|(i, _)| value_types[*i].clone());

            let expanded = quote! {
                type #tag_ident = ( #( #sorted_tag_types ),* );
                type #value_ident = ( #( #reordered_value_types ),* );
            };
            TokenStream::from(expanded)
        }
    }
}
