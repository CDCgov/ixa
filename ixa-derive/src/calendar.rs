//! The `#[calendar]` attribute: see `ixa::calendar`.

use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::ext::IdentExt;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Attribute, Data, DataStruct, DeriveInput, Error, Fields, Ident, LitStr,
    Path, Result, Token,
};

#[derive(Default)]
struct Options {
    inclusive: bool,
    duration: bool,
    signed: bool,
    week: bool,
}

enum Mark {
    None,
    /// `#[calendar(reference)]`: the root's start date.
    Reference,
    /// `#[calendar]` or `#[calendar(..)]` on a type that implements `Calendar`.
    Calendar(Options),
}

fn mark_of(attrs: &mut Vec<Attribute>) -> Result<Mark> {
    let mut mark = Mark::None;
    let mut rest = Vec::with_capacity(attrs.len());
    for attr in attrs.drain(..) {
        if !attr.path().is_ident("calendar") {
            rest.push(attr);
            continue;
        }
        if !matches!(mark, Mark::None) {
            return Err(Error::new_spanned(attr, "a field takes one `#[calendar]`"));
        }
        let mut reference = false;
        let mut options = Options::default();
        if !matches!(attr.meta, syn::Meta::Path(_)) {
            attr.parse_nested_meta(|meta| {
                let flag = if meta.path.is_ident("reference") {
                    &mut reference
                } else if meta.path.is_ident("inclusive") {
                    &mut options.inclusive
                } else if meta.path.is_ident("duration") {
                    &mut options.duration
                } else if meta.path.is_ident("signed") {
                    &mut options.signed
                } else if meta.path.is_ident("week") {
                    &mut options.week
                } else {
                    return Err(meta.error(
                        "expected `reference`, `inclusive`, `duration`, `signed` or `week`",
                    ));
                };
                if std::mem::replace(flag, true) {
                    return Err(meta.error("repeated option"));
                }
                Ok(())
            })?;
        }
        let Options {
            inclusive,
            duration,
            signed,
            week,
        } = options;
        let conflict = if reference && (inclusive || duration || signed || week) {
            Some("`reference` takes no other options")
        } else if duration && (inclusive || week) {
            Some("a `duration` is not a point in time, so it cannot be `inclusive` or `week`")
        } else if signed && !duration {
            Some("`signed` is for a `duration`")
        } else {
            None
        };
        if let Some(message) = conflict {
            return Err(Error::new_spanned(attr, message));
        }
        mark = if reference {
            Mark::Reference
        } else {
            Mark::Calendar(options)
        };
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

pub fn calendar(args: TokenStream, item: TokenStream) -> TokenStream {
    let mut weeks = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("weeks") {
            let value: LitStr = meta.value()?.parse()?;
            weeks = Some(match value.value().as_str() {
                "iso" => format_ident!("Iso"),
                "epi" => format_ident!("Epi"),
                _ => return Err(Error::new_spanned(value, "expected \"iso\" or \"epi\"")),
            });
            Ok(())
        } else {
            Err(meta.error("expected `weeks = \"iso\"` or `weeks = \"epi\"`"))
        }
    });
    parse_macro_input!(args with parser);
    let input = parse_macro_input!(item as DeriveInput);
    match expand(input, weeks) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(mut input: DeriveInput, weeks: Option<Ident>) -> Result<proc_macro2::TokenStream> {
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
    let mut shifts = Vec::new();
    let mut start_shifts = Vec::new();
    let mut checks = Vec::new();

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
                            "`#[calendar(reference)]` goes on one field only",
                        ));
                    }
                    start_shifts.push(quote_spanned! {ty.span()=>
                        <#ty as ::ixa::calendar::StartDate>::shift_start_date(&mut self.#ident, days)
                            .map_err(|e| ::ixa::calendar::CalendarError::at(e, #field_name))?;
                    });
                }
                wire_fields.push(quote! { #(#attrs)* #field_vis #ident: #ty });
                from_wire.push(quote! { #ident: wire.#ident });
                to_wire.push(quote! { #ident: ::std::clone::Clone::clone(&self.#ident) });
            }
            Mark::Calendar(Options {
                inclusive,
                duration,
                signed,
                week,
            }) => {
                reject_custom_serde(attrs, "a `#[calendar]` field is read as its wire form")?;
                // Spanned on the type so a type without `Calendar` is reported there.
                let span = ty.span();
                let field = quote! {
                    ::ixa::calendar::FieldOptions {
                        inclusive: #inclusive,
                        duration: #duration,
                        signed: #signed,
                        write_week: #week,
                    }
                };
                wire_fields.push(quote_spanned! {span=>
                    #(#attrs)* #field_vis #ident: <#ty as ::ixa::calendar::Calendar>::Wire
                });
                from_wire.push(quote_spanned! {span=>
                    #ident: <#ty as ::ixa::calendar::Calendar>::from_calendar(wire.#ident, cx, #field)
                        .map_err(|e| ::ixa::calendar::CalendarError::at(e, #field_name))?
                });
                to_wire.push(quote_spanned! {span=>
                    #ident: <#ty as ::ixa::calendar::Calendar>::to_calendar(&self.#ident, cx, #field)
                });
                if !duration {
                    shifts.push(quote_spanned! {span=>
                        <#ty as ::ixa::calendar::Calendar>::shift_days(&mut self.#ident, days);
                    });
                }
                start_shifts.push(quote_spanned! {span=>
                    <#ty as ::ixa::calendar::Calendar>::shift_start_dates(&mut self.#ident, days)
                        .map_err(|e| ::ixa::calendar::CalendarError::at(e, #field_name))?;
                });
                if inclusive || duration || week {
                    checks.push(quote_spanned! {span=>
                        const _: () = ::std::assert!(
                            <#ty as ::ixa::calendar::Calendar>::DAYS,
                            "`inclusive`, `duration`, `signed` and `week` are for a field of days: an `f64`, or an `Option` or `Vec` of one",
                        );
                    });
                }
            }
        }
    }

    // The twin gets its own doc and keeps `Deserialize` when the struct drops it.
    let twin_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("doc"))
        .cloned()
        .collect();
    // A root uses its own start date, falling back to an enclosing root's, and a struct
    // with a week system uses it for itself and the structs inside it.
    let weeks_expr = match &weeks {
        Some(system) => {
            quote! { ::std::option::Option::Some(::ixa::calendar::WeekSystem::#system) }
        }
        None => quote! { cx.weeks },
    };
    let own_cx = |from: proc_macro2::TokenStream| {
        let start = match &reference {
            Some(start_date) => quote! {
                ::std::option::Option::or(
                    ::std::convert::Into::<::std::option::Option<::ixa::calendar::Date>>::into(
                        ::std::clone::Clone::clone(&#from.#start_date),
                    ),
                    cx.start,
                )
            },
            None => quote! { cx.start },
        };
        quote! {
            let cx = ::ixa::calendar::CalendarContext { start: #start, weeks: #weeks_expr };
        }
    };
    let wire_cx = own_cx(quote!(wire));
    let self_cx = own_cx(quote!(self));
    let week_const = weeks.as_ref().map_or_else(
        || quote!(::std::option::Option::None),
        |w| quote!(::std::option::Option::Some(::ixa::calendar::WeekSystem::#w)),
    );
    let root = reference.is_some().then(|| {
        quote! {
            impl ::std::convert::TryFrom<#wire> for #name {
                type Error = ::ixa::calendar::CalendarError;

                fn try_from(wire: #wire) -> ::std::result::Result<Self, Self::Error> {
                    <Self as ::ixa::calendar::Calendar>::from_calendar(
                        wire,
                        ::std::default::Default::default(),
                        ::std::default::Default::default(),
                    )
                }
            }

            impl #name {
                /// The week system set by `#[calendar(weeks = ..)]`, for `set_week_system`.
                pub const WEEK_SYSTEM: ::std::option::Option<::ixa::calendar::WeekSystem> = #week_const;

                /// The twin with whole days as dates.
                #[must_use]
                pub fn dated(&self) -> #wire {
                    <Self as ::ixa::calendar::Calendar>::to_calendar(
                        self,
                        ::std::default::Default::default(),
                        ::std::default::Default::default(),
                    )
                }
            }
        }
    });
    // A struct read on its own reads its twin in human-readable formats and its own
    // layout, days only, in binary ones, so the bytes match what the derived `Serialize`
    // writes. Nested in another calendar struct, it is read as its twin instead.
    let (found, has_serialize) = strip_derive_deserialize(&mut input.attrs)?;
    if found && !has_serialize {
        // No serde derive is left to own the struct's serde attributes; the twin and the
        // days struct already carry their copies.
        input.attrs.retain(|attr| !attr.path().is_ident("serde"));
        for field in fields.iter_mut() {
            field.attrs.retain(|attr| !attr.path().is_ident("serde"));
        }
    }
    let deserialize = found.then(|| {
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
                            <Self as ::ixa::calendar::Calendar>::from_calendar(
                                wire,
                                ::std::default::Default::default(),
                                ::std::default::Default::default(),
                            )
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

            #[allow(unused_variables)]
            fn from_calendar(
                wire: #wire,
                cx: ::ixa::calendar::CalendarContext,
                _field: ::ixa::calendar::FieldOptions,
            ) -> ::std::result::Result<Self, ::ixa::calendar::CalendarError> {
                #wire_cx
                ::std::result::Result::Ok(Self { #(#from_wire,)* })
            }

            #[allow(unused_variables)]
            fn to_calendar(
                &self,
                cx: ::ixa::calendar::CalendarContext,
                _field: ::ixa::calendar::FieldOptions,
            ) -> #wire {
                #self_cx
                #wire { #(#to_wire,)* }
            }

            #[allow(unused_variables)]
            fn shift_days(&mut self, days: f64) {
                #(#shifts)*
            }

            #[allow(unused_variables)]
            fn shift_start_dates(
                &mut self,
                days: i64,
            ) -> ::std::result::Result<(), ::ixa::calendar::CalendarError> {
                #(#start_shifts)*
                ::std::result::Result::Ok(())
            }
        }

        #(#checks)*

        #root
        #deserialize
    })
}
