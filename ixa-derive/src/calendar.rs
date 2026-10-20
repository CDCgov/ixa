//! The `#[calendar]` attribute: see `ixa::calendar`.

use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::ext::IdentExt;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Attribute, Data, DataStruct, DeriveInput, Error, Fields, Ident, Path,
    Result, Token, Type,
};

enum Mark {
    None,
    /// `#[day]` or `#[day(inclusive)]` on an `f64` or `Option<f64>`.
    Day {
        inclusive: bool,
    },
    /// `#[days]` on a type that implements `Calendar`.
    Days,
    /// `#[reference]`: the root's start date.
    Reference,
}

fn mark_of(attrs: &mut Vec<Attribute>) -> Result<Mark> {
    let mut mark = Mark::None;
    let mut rest = Vec::with_capacity(attrs.len());
    for attr in attrs.drain(..) {
        let path = attr.path();
        let found = if path.is_ident("day") {
            let mut inclusive = false;
            if let syn::Meta::List(_) = &attr.meta {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("inclusive") {
                        inclusive = true;
                        Ok(())
                    } else {
                        Err(meta.error("expected `#[day]` or `#[day(inclusive)]`"))
                    }
                })?;
            }
            Some(Mark::Day { inclusive })
        } else if path.is_ident("days") {
            Some(Mark::Days)
        } else if path.is_ident("reference") {
            Some(Mark::Reference)
        } else {
            None
        };
        match found {
            Some(found) if matches!(mark, Mark::None) => mark = found,
            Some(_) => {
                return Err(Error::new_spanned(
                    attr,
                    "a field takes one of `#[day]`, `#[days]` and `#[reference]`",
                ))
            }
            None => rest.push(attr),
        }
    }
    *attrs = rest;
    Ok(mark)
}

/// The keys of a `#[serde(..)]` attribute, with whether each has a value;
/// empty for any other attribute.
fn serde_keys(attr: &Attribute) -> Result<Vec<(String, bool)>> {
    let mut keys = Vec::new();
    if attr.path().is_ident("serde") {
        attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .get_ident()
                .map(ToString::to_string)
                .unwrap_or_default();
            let has_value = meta.input.peek(syn::Token![=]);
            if has_value {
                meta.value()?.parse::<proc_macro2::TokenTree>()?;
            } else if meta.input.peek(syn::token::Paren) {
                meta.input.parse::<proc_macro2::TokenTree>()?;
            }
            keys.push((key, has_value));
            Ok(())
        })?;
    }
    Ok(keys)
}

/// Removes `Deserialize` from the `#[derive(..)]` attributes. Returns whether it was there
/// and whether a `Serialize` derive remains to own the `#[serde(..)]` attributes.
fn strip_derive_deserialize(attrs: &mut Vec<Attribute>) -> Result<(bool, bool)> {
    let mut found = false;
    let mut has_serialize = false;
    let mut rest = Vec::with_capacity(attrs.len());
    for attr in attrs.drain(..) {
        if !attr.path().is_ident("derive") {
            rest.push(attr);
            continue;
        }
        let paths = attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
        let kept: Vec<Path> = paths
            .into_iter()
            .filter(|path| {
                let last = path.segments.last().map(|segment| &segment.ident);
                let is_deserialize = last.is_some_and(|ident| ident == "Deserialize");
                found |= is_deserialize;
                has_serialize |= last.is_some_and(|ident| ident == "Serialize");
                !is_deserialize
            })
            .collect();
        if !kept.is_empty() {
            let span = attr.span();
            rest.push(syn::parse_quote_spanned!(span => #[derive(#(#kept),*)]));
        }
    }
    *attrs = rest;
    Ok((found, has_serialize))
}

/// Rejects serde attributes that would type a twin field as the model type.
fn reject_custom_serde(attrs: &[Attribute], reason: &str) -> Result<()> {
    for attr in attrs {
        for (key, has_value) in serde_keys(attr)? {
            let custom = ["with", "deserialize_with", "serialize_with"].contains(&key.as_str())
                || (key == "default" && has_value);
            if custom {
                return Err(Error::new_spanned(
                    attr,
                    format!("{reason}, so serde `{key}` cannot be used on it"),
                ));
            }
        }
    }
    Ok(())
}

/// Whether `ty` is written `Option<..>` (by its last path segment).
fn is_option(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Option"),
        _ => false,
    }
}

pub fn calendar(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return Error::new_spanned(
            proc_macro2::TokenStream::from(args),
            "`#[calendar]` takes no arguments",
        )
        .to_compile_error()
        .into();
    }
    let input = parse_macro_input!(item as DeriveInput);
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(mut input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &input.generics,
            "`#[calendar]` does not support generics",
        ));
    }
    let Data::Struct(DataStruct {
        fields: Fields::Named(named),
        ..
    }) = &mut input.data
    else {
        return Err(Error::new_spanned(
            &input.ident,
            "`#[calendar]` needs a struct with named fields",
        ));
    };
    let fields = &mut named.named;
    // The twin copies the derives, which must sit below this attribute.
    if !input
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("derive"))
    {
        return Err(Error::new_spanned(
            &input.ident,
            "`#[calendar]` copies the struct's derives onto its twin, so it goes above `#[derive(..)]`",
        ));
    }

    for attr in &input.attrs {
        for (key, _) in serde_keys(attr)? {
            if ["try_from", "from", "into", "remote"].contains(&key.as_str()) {
                return Err(Error::new_spanned(
                    attr,
                    format!("`#[calendar]` generates the wire form, so serde `{key}` cannot be used; implement `Calendar` by hand instead"),
                ));
            }
        }
    }

    let name = input.ident.clone();
    let wire = format_ident!("Calendar{}", name);
    let vis = input.vis.clone();
    let mut reference: Option<Ident> = None;
    let mut wire_fields = Vec::new();
    let mut from_wire = Vec::new();
    let mut to_wire = Vec::new();
    let mut field_idents = Vec::new();
    let mut days_fields = Vec::new();

    for field in fields.iter_mut() {
        let ident = field.ident.clone().expect("named field");
        let field_name = ident.unraw().to_string();
        let mark = mark_of(&mut field.attrs)?;
        let attrs = &field.attrs;
        let field_vis = &field.vis;
        let ty = &field.ty;
        field_idents.push(ident.clone());
        days_fields.push(quote! { #(#attrs)* #ident: #ty });
        match mark {
            Mark::None | Mark::Reference => {
                if let Mark::Reference = mark {
                    if reference.replace(ident.clone()).is_some() {
                        return Err(Error::new_spanned(
                            ident,
                            "`#[reference]` goes on one field only",
                        ));
                    }
                }
                wire_fields.push(quote! { #(#attrs)* #field_vis #ident: #ty });
                from_wire.push(quote! { #ident: wire.#ident });
                to_wire.push(quote! { #ident: ::std::clone::Clone::clone(&self.#ident) });
            }
            Mark::Day { inclusive } => {
                reject_custom_serde(attrs, "a `#[day]` field is read as `Day`")?;
                // Spanned on the type so a non-`f64` field is reported there.
                let span = ty.span();
                if is_option(ty) {
                    wire_fields.push(quote! {
                        #(#attrs)* #field_vis #ident: ::std::option::Option<::ixa::calendar::Day>
                    });
                    from_wire.push(quote_spanned! {span=>
                        #ident: wire.#ident
                            .map(|day| ::ixa::calendar::Day::resolve(day, start, #inclusive))
                            .transpose()
                            .map_err(|e| ::ixa::calendar::CalendarError::at(e, #field_name))?
                    });
                    to_wire.push(quote_spanned! {span=>
                        #ident: self.#ident
                            .map(|days| ::ixa::calendar::Day::from_days(days, start, #inclusive))
                    });
                } else {
                    wire_fields.push(quote! {
                        #(#attrs)* #field_vis #ident: ::ixa::calendar::Day
                    });
                    from_wire.push(quote_spanned! {span=>
                        #ident: ::ixa::calendar::Day::resolve(wire.#ident, start, #inclusive)
                            .map_err(|e| ::ixa::calendar::CalendarError::at(e, #field_name))?
                    });
                    to_wire.push(quote_spanned! {span=>
                        #ident: ::ixa::calendar::Day::from_days(self.#ident, start, #inclusive)
                    });
                }
            }
            Mark::Days => {
                reject_custom_serde(attrs, "a `#[days]` field is read as its twin")?;
                wire_fields.push(quote! {
                    #(#attrs)* #field_vis #ident: <#ty as ::ixa::calendar::Calendar>::Wire
                });
                from_wire.push(quote! {
                    #ident: <#ty as ::ixa::calendar::Calendar>::from_calendar(wire.#ident, start)
                        .map_err(|e| ::ixa::calendar::CalendarError::at(e, #field_name))?
                });
                to_wire.push(quote! {
                    #ident: ::ixa::calendar::Calendar::to_calendar(&self.#ident, start)
                });
            }
        }
    }

    // The twin gets its own doc and keeps `Deserialize` when the root drops it.
    let twin_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("doc"))
        .cloned()
        .collect();
    // A root uses its own start date, falling back to an enclosing root's.
    let own_start = |from: proc_macro2::TokenStream| {
        reference.as_ref().map(|start_date| {
            quote! {
                let start = ::std::option::Option::or(
                    ::std::convert::Into::<::std::option::Option<::ixa::calendar::Date>>::into(
                        ::std::clone::Clone::clone(&#from.#start_date),
                    ),
                    start,
                );
            }
        })
    };
    let wire_start = own_start(quote!(wire));
    let self_start = own_start(quote!(self));
    let root = reference.is_some().then(|| {
        quote! {
            impl ::std::convert::TryFrom<#wire> for #name {
                type Error = ::ixa::calendar::CalendarError;

                fn try_from(wire: #wire) -> ::std::result::Result<Self, Self::Error> {
                    <Self as ::ixa::calendar::Calendar>::from_calendar(wire, ::std::option::Option::None)
                }
            }

            impl #name {
                /// The twin with whole days as dates.
                #[must_use]
                pub fn dated(&self) -> #wire {
                    <Self as ::ixa::calendar::Calendar>::to_calendar(self, ::std::option::Option::None)
                }
            }
        }
    });
    // A root reads its twin in human-readable formats and its own layout, days only, in
    // binary ones, so the bytes match what the derived `Serialize` writes.
    let root_deserialize = if reference.is_some() {
        let (found, has_serialize) = strip_derive_deserialize(&mut input.attrs)?;
        if found && !has_serialize {
            // No serde derive is left to own the root's serde attributes; the twin and the
            // days struct already carry their copies.
            input.attrs.retain(|attr| !attr.path().is_ident("serde"));
            for field in fields.iter_mut() {
                field.attrs.retain(|attr| !attr.path().is_ident("serde"));
            }
        }
        found
    } else {
        false
    };
    let root_deserialize = root_deserialize.then(|| {
        let days = format_ident!("Days{}", name);
        quote! {
            const _: () = {
                #[allow(dead_code)]
                #(#twin_attrs)*
                struct #days {
                    #(#days_fields,)*
                }

                impl<'de> ::ixa::__private::serde::Deserialize<'de> for #name {
                    fn deserialize<D: ::ixa::__private::serde::Deserializer<'de>>(
                        deserializer: D,
                    ) -> ::std::result::Result<Self, D::Error> {
                        if ::ixa::__private::serde::Deserializer::is_human_readable(&deserializer) {
                            let wire = <#wire as ::ixa::__private::serde::Deserialize>::deserialize(deserializer)?;
                            <Self as ::std::convert::TryFrom<#wire>>::try_from(wire)
                                .map_err(::ixa::__private::serde::de::Error::custom)
                        } else {
                            let days = <#days as ::ixa::__private::serde::Deserialize>::deserialize(deserializer)?;
                            ::std::result::Result::Ok(Self { #(#field_idents: days.#field_idents,)* })
                        }
                    }
                }
            };
        }
    });

    let doc = format!("[`{name}`] with days or dates, generated by `#[calendar]`.");
    Ok(quote! {
        #input

        #[doc = #doc]
        #(#twin_attrs)*
        #vis struct #wire {
            #(#wire_fields,)*
        }

        impl ::ixa::calendar::Calendar for #name {
            type Wire = #wire;

            fn from_calendar(
                wire: #wire,
                start: ::std::option::Option<::ixa::calendar::Date>,
            ) -> ::std::result::Result<Self, ::ixa::calendar::CalendarError> {
                #wire_start
                ::std::result::Result::Ok(Self { #(#from_wire,)* })
            }

            fn to_calendar(&self, start: ::std::option::Option<::ixa::calendar::Date>) -> #wire {
                #self_start
                #wire { #(#to_wire,)* }
            }
        }

        #root
        #root_deserialize
    })
}
