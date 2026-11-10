extern crate proc_macro;

mod calendar;
mod eq_hash;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, DeriveInput};

/// Marks a struct's calendar day fields; see `ixa::calendar`.
#[proc_macro_attribute]
pub fn calendar(args: TokenStream, item: TokenStream) -> TokenStream {
    calendar::calendar(args, item)
}

/// Field-by-field `PartialEq`, `Eq` and `Hash` with floats compared by value; documented at
/// `ixa::EqHash`.
#[proc_macro_derive(EqHash, attributes(eq_hash))]
pub fn derive_eq_hash(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    eq_hash::derive_eq_hash(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(IxaEvent)]
pub fn derive_ixa_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let self_ty = quote! { #name #ty_generics };
    let mut event_generics = generics.clone();
    event_generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#self_ty: 'static));
    let (event_impl_generics, _, event_where_clause) = event_generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics ::std::clone::Clone for #self_ty #where_clause {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl #impl_generics ::std::marker::Copy for #self_ty #where_clause {}
        impl #event_impl_generics IxaEvent for #self_ty #event_where_clause {}
    };

    TokenStream::from(expanded)
}
