extern crate proc_macro;
mod reorder_fn;
mod sorted_tag;
mod utilities;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(IxaEvent)]
pub fn derive_ixa_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl IxaEvent for #name {}
    };

    TokenStream::from(expanded)
}

#[proc_macro]
pub fn sorted_tag(input: TokenStream) -> TokenStream {
    sorted_tag::sorted_tag(input)
}

#[proc_macro]
pub fn sorted_value_type(input: TokenStream) -> TokenStream {
    sorted_tag::sorted_value_type(input)
}

#[proc_macro]
pub fn impl_make_canonical(input: TokenStream) -> TokenStream {
    reorder_fn::impl_reorder_fns(input)
}
