//! The `EqHash` derive: see `ixa::EqHash`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Member, Type};

pub fn derive_eq_hash(input: &DeriveInput) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "EqHash does not support generic types; implement PartialEq, Eq and Hash by hand",
        ));
    }
    let name = &input.ident;

    // A struct is handled as an enum with one variant whose path is `Self`.
    let variants: Vec<(TokenStream, &Fields)> = match &input.data {
        Data::Struct(data) => vec![(quote! { Self }, &data.fields)],
        Data::Enum(data) => {
            if data.variants.is_empty() {
                return Err(syn::Error::new_spanned(
                    &data.variants,
                    "EqHash requires at least one variant",
                ));
            }
            data.variants
                .iter()
                .map(|variant| {
                    let variant_name = &variant.ident;
                    (quote! { Self::#variant_name }, &variant.fields)
                })
                .collect()
        }
        Data::Union(data) => {
            return Err(syn::Error::new_spanned(
                data.union_token,
                "EqHash cannot be derived for unions",
            ))
        }
    };

    let arms: Vec<VariantArms> = variants
        .iter()
        .map(|(path, fields)| variant_arms(path, fields))
        .collect();
    let eq_arms = arms.iter().map(|arms| &arms.eq);
    let hash_arms = arms.iter().map(|arms| &arms.hash);
    // Only reached with more than one variant, otherwise the pattern would be unreachable.
    let different_variants = (variants.len() > 1).then(|| quote! { _ => false, });
    // As in the std derive, the discriminant is hashed only when there is a choice.
    let hash_discriminant = (variants.len() > 1).then(|| {
        quote! { ::core::hash::Hash::hash(&::core::mem::discriminant(self), state); }
    });

    Ok(quote! {
        #[automatically_derived]
        impl ::core::cmp::PartialEq for #name {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                #[allow(unused_imports)]
                use ::ixa::value_key::{KeyProbeFallback, KeyProbeValue};
                match (self, other) {
                    #(#eq_arms)*
                    #different_variants
                }
            }
        }

        #[automatically_derived]
        impl ::core::cmp::Eq for #name {}

        #[automatically_derived]
        impl ::core::hash::Hash for #name {
            #[inline]
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                #[allow(unused_imports)]
                use ::ixa::value_key::{KeyProbeFallback, KeyProbeValue};
                #hash_discriminant
                match self {
                    #(#hash_arms)*
                }
            }
        }
    })
}

struct VariantArms {
    /// `(Path { f: left_0, .. }, Path { f: right_0, .. }) => <all fields equal>,`
    eq: TokenStream,
    /// `Path { f: left_0, .. } => { <hash each field> }`
    hash: TokenStream,
}

fn variant_arms(path: &TokenStream, fields: &Fields) -> VariantArms {
    let mut members = Vec::new();
    let mut field_types = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        members.push(match &field.ident {
            Some(ident) => Member::Named(ident.clone()),
            None => Member::Unnamed(index.into()),
        });
        field_types.push(&field.ty);
    }
    let left_names: Vec<_> = (0..members.len())
        .map(|index| format_ident!("left_{index}"))
        .collect();
    let right_names: Vec<_> = (0..members.len())
        .map(|index| format_ident!("right_{index}"))
        .collect();
    let left_keys: Vec<_> = field_types
        .iter()
        .zip(&left_names)
        .map(|(field_type, name)| key_of(field_type, name))
        .collect();
    let right_keys: Vec<_> = field_types
        .iter()
        .zip(&right_names)
        .map(|(field_type, name)| key_of(field_type, name))
        .collect();

    let all_equal = if left_keys.is_empty() {
        quote! { true }
    } else {
        quote! { #(#left_keys == #right_keys)&&* }
    };
    // The `Path { member: binding }` pattern form covers named, tuple and unit shapes alike.
    VariantArms {
        eq: quote! {
            (
                #path { #(#members: #left_names),* },
                #path { #(#members: #right_names),* },
            ) => #all_equal,
        },
        hash: quote! {
            #path { #(#members: #left_names),* } => {
                #(::core::hash::Hash::hash(&#left_keys, state);)*
            }
        },
    }
}

/// The key of a field, resolved at the expansion site: its `ValueKey` when the field type has
/// one (floats by canonical bits), otherwise a reference to the value, compared and hashed
/// through its own `Eq` and `Hash`.
fn key_of(field_type: &Type, binding: &syn::Ident) -> TokenStream {
    quote! {
        (&::ixa::value_key::KeyProbe::<#field_type>::new()).probe_key(#binding)
    }
}
