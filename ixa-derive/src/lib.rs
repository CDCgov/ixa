extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Fields, Ident};

#[proc_macro_derive(IxaEvent)]
pub fn derive_ixa_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl IxaEvent for #name {}
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(IxaParams)]
pub fn derive_ixa_params(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);

    let name = &ast.ident;
    let args_name = syn::Ident::new(&format!("{}Options", name), name.span());

    let struct_attrs: Vec<&Attribute> = ast.attrs.iter().collect();

    let fields = match &mut ast.data {
        Data::Struct(data) => match &mut data.fields {
            Fields::Named(fields) => {
                let modified_fields = fields.named.iter().map(|f| {
                    let field_name = &f.ident;
                    let field_type = &f.ty;
                    let modified_type = quote! { Option<#field_type> };
                    let mut field_attrs = f.attrs.clone();

                    let has_clap_attr = field_attrs.iter().any(|attr| attr.path().is_ident("clap"));
                    if !has_clap_attr {
                        field_attrs.push(syn::parse_quote!(#[clap(long)]));
                    }

                    quote! {
                        #(#field_attrs)*
                        pub #field_name: #modified_type,
                    }
                });
                quote! { #(#modified_fields)* }
            }
            Fields::Unit => quote! {},
            _ => panic!("IxaParams only supports structs with named fields"),
        },
        _ => panic!("IxaParams can only be derived on structs"),
    };

    let field_names: Vec<&Ident> = match &ast.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap())
                .collect(),
            _ => vec![],
        },
        _ => vec![],
    };

    let merge_config_fields = field_names.iter().map(|field| {
        quote! {
            #field: self.#field.clone().unwrap_or_else(|| other.#field),
        }
    });

    quote! {
        #[derive(clap::Parser)]
        #[command(version, about)]
        struct #args_name {
            #fields
        }

        impl ixa::global_properties::IxaParamsArgs<#name> for #args_name {
            fn parse() -> Self {
                <Self as Parser>::parse()
            }
            fn merge_config(&self, other: #name) -> Result<#name, IxaError> {
                Ok(#name {
                    #(#merge_config_fields)*
                })
            }
        }

        impl ixa::global_properties::IxaParams<#args_name> for #name {
            fn name() -> &'static str {
                stringify!(#name)
            }
        }
    }
    .into()
}
