//! The `#[calendar]` attribute: see `ixa::calendar`.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, parse_quote, Attribute, Data, DeriveInput, Error, Fields, Ident, Result,
    Type,
};

enum Mark {
    None,
    /// `#[day]` or `#[day(through)]` on an `f64` or `Option<f64>`.
    Day {
        through: bool,
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
            let mut through = false;
            if let syn::Meta::List(_) = &attr.meta {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("through") {
                        through = true;
                        Ok(())
                    } else {
                        Err(meta.error("expected `#[day]` or `#[day(through)]`"))
                    }
                })?;
            }
            Some(Mark::Day { through })
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
    let fields = match &mut input.data {
        Data::Struct(data) => match &mut data.fields {
            Fields::Named(named) => &mut named.named,
            _ => {
                return Err(Error::new_spanned(
                    &input.ident,
                    "`#[calendar]` needs a struct with named fields",
                ))
            }
        },
        _ => {
            return Err(Error::new_spanned(
                &input.ident,
                "`#[calendar]` needs a struct with named fields",
            ))
        }
    };

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

    for field in fields.iter_mut() {
        let ident = field.ident.clone().expect("named field");
        let mark = mark_of(&mut field.attrs)?;
        let attrs = &field.attrs;
        let field_vis = &field.vis;
        let ty = &field.ty;
        match mark {
            Mark::None => {
                wire_fields.push(quote! { #(#attrs)* #field_vis #ident: #ty });
                from_wire.push(quote! { #ident: wire.#ident });
                to_wire.push(quote! { #ident: ::std::clone::Clone::clone(&self.#ident) });
            }
            Mark::Reference => {
                if reference.is_some() {
                    return Err(Error::new_spanned(
                        ident,
                        "`#[reference]` goes on one field only",
                    ));
                }
                reference = Some(ident.clone());
                wire_fields.push(quote! { #(#attrs)* #field_vis #ident: #ty });
                from_wire.push(quote! { #ident: wire.#ident });
                to_wire.push(quote! { #ident: ::std::clone::Clone::clone(&self.#ident) });
            }
            Mark::Day { through } => {
                for attr in attrs {
                    for (key, has_value) in serde_keys(attr)? {
                        let custom = ["with", "deserialize_with", "serialize_with"]
                            .contains(&key.as_str())
                            || (key == "default" && has_value);
                        if custom {
                            return Err(Error::new_spanned(
                                attr,
                                format!("a `#[day]` field is read as `Day`, so serde `{key}` cannot be used on it"),
                            ));
                        }
                    }
                }
                if is_option(ty) {
                    wire_fields.push(quote! {
                        #(#attrs)* #field_vis #ident: ::std::option::Option<::ixa::calendar::Day>
                    });
                    from_wire.push(quote! {
                        #ident: wire.#ident
                            .map(|day| ::ixa::calendar::Day::resolve(day, start, #through))
                            .transpose()?
                    });
                    to_wire.push(quote! {
                        #ident: self.#ident
                            .map(|days| ::ixa::calendar::Day::from_days(days, start, #through))
                    });
                } else {
                    wire_fields.push(quote! {
                        #(#attrs)* #field_vis #ident: ::ixa::calendar::Day
                    });
                    from_wire.push(quote! {
                        #ident: ::ixa::calendar::Day::resolve(wire.#ident, start, #through)?
                    });
                    to_wire.push(quote! {
                        #ident: ::ixa::calendar::Day::from_days(self.#ident, start, #through)
                    });
                }
            }
            Mark::Days => {
                wire_fields.push(quote! {
                    #(#attrs)* #field_vis #ident: <#ty as ::ixa::calendar::Calendar>::Wire
                });
                from_wire.push(quote! {
                    #ident: <#ty as ::ixa::calendar::Calendar>::from_calendar(wire.#ident, start)?
                });
                to_wire.push(quote! {
                    #ident: ::ixa::calendar::Calendar::to_calendar(&self.#ident, start)
                });
            }
        }
    }

    // Taken before the root's `try_from` is added; the twin gets its own doc.
    let twin_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("doc"))
        .cloned()
        .collect();
    let root = reference.map(|start_date| {
        let wire_name = wire.to_string();
        input
            .attrs
            .push(parse_quote!(#[serde(try_from = #wire_name)]));
        quote! {
            impl ::std::convert::TryFrom<#wire> for #name {
                type Error = ::ixa::calendar::CalendarError;

                fn try_from(wire: #wire) -> ::std::result::Result<Self, Self::Error> {
                    let start: ::std::option::Option<::ixa::calendar::Date> =
                        ::std::clone::Clone::clone(&wire.#start_date).into();
                    <Self as ::ixa::calendar::Calendar>::from_calendar(wire, start)
                }
            }

            impl #name {
                /// The twin with whole days as dates.
                #[must_use]
                pub fn dated(&self) -> #wire {
                    let start: ::std::option::Option<::ixa::calendar::Date> =
                        ::std::clone::Clone::clone(&self.#start_date).into();
                    <Self as ::ixa::calendar::Calendar>::to_calendar(self, start)
                }
            }
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
                ::std::result::Result::Ok(Self { #(#from_wire,)* })
            }

            fn to_calendar(&self, start: ::std::option::Option<::ixa::calendar::Date>) -> #wire {
                #wire { #(#to_wire,)* }
            }
        }

        #root
    })
}
