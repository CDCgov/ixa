//! The `EqHash` derive: see `ixa::EqHash`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Fields, Member, Path, Type};

pub fn derive_eq_hash(input: &DeriveInput) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "EqHash does not support generic types; implement PartialEq, Eq and Hash by hand",
        ));
    }
    let name = &input.ident;
    let ixa = crate_path(&input.attrs)?;

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
        .map(|(path, fields)| variant_arms(&ixa, path, fields))
        .collect();
    let eq_arms = arms.iter().map(|arms| &arms.eq);
    let hash_arms = arms.iter().map(|arms| &arms.hash);
    // Only reached with more than one variant, otherwise the pattern would be unreachable.
    let different_variants = (variants.len() > 1).then(|| quote! { _ => false, });
    // As in the std derive, the discriminant is hashed only when there is a choice.
    let hash_discriminant = (variants.len() > 1).then(|| {
        quote! { ::core::hash::Hash::hash(&::core::mem::discriminant(self), state); }
    });

    Ok(impls(
        name,
        &ixa,
        &quote! {
            match (self, other) {
                #(#eq_arms)*
                #different_variants
            }
        },
        &quote! {
            #hash_discriminant
            match self {
                #(#hash_arms)*
            }
        },
    ))
}

fn impls(
    name: &syn::Ident,
    ixa: &Path,
    eq_body: &TokenStream,
    hash_body: &TokenStream,
) -> TokenStream {
    quote! {
        #[automatically_derived]
        impl ::core::cmp::PartialEq for #name {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                #[allow(unused_imports)]
                use #ixa::value_key::{KeyProbeFallback, KeyProbeValue};
                #eq_body
            }
        }

        #[automatically_derived]
        impl ::core::cmp::Eq for #name {}

        #[automatically_derived]
        impl ::core::hash::Hash for #name {
            #[inline]
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                #[allow(unused_imports)]
                use #ixa::value_key::{KeyProbeFallback, KeyProbeValue};
                #hash_body
            }
        }
    }
}

/// The path of the ixa crate: `#[eq_hash(crate = path)]` if given (the property macros pass
/// `$crate`, so a renamed or re-exported ixa works), otherwise `::ixa`.
fn crate_path(attrs: &[Attribute]) -> syn::Result<Path> {
    let mut path = None;
    for attr in attrs {
        if !attr.path().is_ident("eq_hash") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                path = Some(meta.value()?.parse::<Path>()?);
                Ok(())
            } else {
                Err(meta.error("expected `crate = path`"))
            }
        })?;
    }
    Ok(path.unwrap_or_else(|| syn::parse_quote!(::ixa)))
}

struct VariantArms {
    /// `(Path { f: __left_0, .. }, Path { f: __right_0, .. }) => <all fields equal>,`
    eq: TokenStream,
    /// `Path { f: __left_0, .. } => { <hash each field> }`
    hash: TokenStream,
}

/// The bindings for one variant's fields: `__left_N` and `__right_N` are references to field N of
/// the two values compared, and the keys are what is compared and hashed.
struct FieldBindings {
    members: Vec<Member>,
    left_names: Vec<syn::Ident>,
    right_names: Vec<syn::Ident>,
    left_keys: Vec<TokenStream>,
    right_keys: Vec<TokenStream>,
}

impl FieldBindings {
    fn new(ixa: &Path, fields: &Fields) -> Self {
        let mut bindings = FieldBindings {
            members: Vec::new(),
            left_names: Vec::new(),
            right_names: Vec::new(),
            left_keys: Vec::new(),
            right_keys: Vec::new(),
        };
        for (index, field) in fields.iter().enumerate() {
            let left_name = format_ident!("__left_{index}");
            let right_name = format_ident!("__right_{index}");
            bindings.members.push(match &field.ident {
                Some(ident) => Member::Named(ident.clone()),
                None => Member::Unnamed(index.into()),
            });
            bindings.left_keys.push(key_of(ixa, &field.ty, &left_name));
            bindings
                .right_keys
                .push(key_of(ixa, &field.ty, &right_name));
            bindings.left_names.push(left_name);
            bindings.right_names.push(right_name);
        }
        bindings
    }

    fn all_equal(&self) -> TokenStream {
        let FieldBindings {
            left_keys,
            right_keys,
            ..
        } = self;
        if left_keys.is_empty() {
            quote! { true }
        } else {
            quote! { #(#left_keys == #right_keys)&&* }
        }
    }
}

fn variant_arms(ixa: &Path, path: &TokenStream, fields: &Fields) -> VariantArms {
    let bindings = FieldBindings::new(ixa, fields);
    let FieldBindings {
        members,
        left_names,
        right_names,
        left_keys,
        ..
    } = &bindings;
    let all_equal = bindings.all_equal();
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
fn key_of(ixa: &Path, field_type: &Type, binding: &syn::Ident) -> TokenStream {
    quote! {
        (&#ixa::value_key::KeyProbe::<#field_type>::new()).probe_key(#binding)
    }
}
