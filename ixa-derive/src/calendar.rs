//! The `#[calendar]` attribute: see `ixa::calendar`.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, parse_quote, Attribute, Data, DeriveInput, Error, Fields, Ident, Result,
    Type,
};

/// How a field takes part in the calendar.
enum Mark {
    /// A plain field, copied as is.
    None,
    /// `#[day]` or `#[day(through)]`: an `f64` or `Option<f64>` of days.
    Day { through: bool },
    /// `#[days]`: a type that implements `Calendar` (a struct, or a `Vec`
    /// or `Option` of one).
    Days,
    /// `#[reference]`: the start date, on the root struct only.
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

/// Whether `ty` is written `Option<..>`.
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

    // The twin carries the struct's attributes (derives, serde) as they
    // were written, before the root's `try_from` is added below.
    let twin_attrs = input.attrs.clone();
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
                /// The wire form with whole days as dates, on this start date.
                #[must_use]
                pub fn dated(&self) -> #wire {
                    let start: ::std::option::Option<::ixa::calendar::Date> =
                        ::std::clone::Clone::clone(&self.#start_date).into();
                    <Self as ::ixa::calendar::Calendar>::to_calendar(self, start)
                }
            }
        }
    });

    let doc = format!(
        "The wire form of [`{name}`]: the same with [`Day`](::ixa::calendar::Day) for \
         each calendar day field. Generated by `#[calendar]`."
    );
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
