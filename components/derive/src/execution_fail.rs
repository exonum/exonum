// Copyright 2020 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use darling::{FromDeriveInput, FromMeta};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use syn::{Attribute, Data, DeriveInput, Expr, Lit, Meta, MetaNameValue, Variant};

use std::convert::TryFrom;

use super::{find_meta_attrs, MainCratePath};

#[derive(Debug, FromMeta)]
#[darling(default)]
struct ExecutionFailAttrs {
    #[darling(rename = "crate")]
    cr: MainCratePath,
    kind: String,
}

impl Default for ExecutionFailAttrs {
    fn default() -> Self {
        Self {
            cr: MainCratePath::default(),
            kind: "Service".to_owned(),
        }
    }
}

impl TryFrom<&[Attribute]> for ExecutionFailAttrs {
    type Error = darling::Error;

    fn try_from(args: &[Attribute]) -> Result<Self, Self::Error> {
        find_meta_attrs("execution_fail", args)
            .map(|meta| {
                Self::from_nested_meta(&meta).and_then(|mut attrs| match attrs.kind.as_str() {
                    "service" | "runtime" | "core" | "common" => {
                        attrs.kind[..1].make_ascii_uppercase();
                        Ok(attrs)
                    }
                    _ => {
                        let msg = "ExecutionFail: Unsupported error kind. Use one of \
                                   \"service\", \"runtime\", or \"core\"";
                        Err(darling::Error::custom(msg))
                    }
                })
            })
            .unwrap_or_else(|| Ok(Self::default()))
    }
}

#[derive(Debug)]
struct ParsedVariant {
    id: Expr,
    ident: Ident,
    comment: String,
}

impl ParsedVariant {
    fn from_variant(variant: &Variant) -> Self {
        assert!(
            variant.fields.iter().len() == 0,
            "ExecutionFail: Each enum variant should not have fields inside."
        );
        let discriminant = variant
            .discriminant
            .clone()
            .expect(
                "ExecutionFail: Each enum variant should have an explicit discriminant declaration",
            )
            .1;
        // TODO parse discriminant.
        let id = discriminant;
        let comment = Self::parse_doc_comment(&variant.attrs);

        ParsedVariant {
            id,
            ident: variant.ident.clone(),
            comment,
        }
    }

    // This method was been inspired by the `structopt-derive::push_doc_comment`
    fn parse_doc_comment(attrs: &[Attribute]) -> String {
        let doc_comments = attrs
            .iter()
            .filter_map(|attr| {
                let path = &attr.path;
                if quote!(#path).to_string() == "doc" {
                    attr.parse_meta().ok()
                } else {
                    None
                }
            })
            .filter_map(|attr| {
                if let Meta::NameValue(MetaNameValue {
                    path,
                    lit: Lit::Str(s),
                    ..
                }) = attr
                {
                    if !path.is_ident("doc") {
                        return None;
                    }
                    let value = s.value();
                    let text = value
                        .trim_start_matches("//!")
                        .trim_start_matches("///")
                        .trim_start_matches("/*!")
                        .trim_start_matches("/**")
                        .trim_end_matches("*/")
                        .trim();
                    if text.is_empty() {
                        Some("\n\n".to_string())
                    } else {
                        Some(text.to_string())
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        doc_comments
            .join(" ")
            .split('\n')
            .map(str::trim)
            .map(str::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug)]
struct ExecutionFail {
    name: Ident,
    variants: Vec<ParsedVariant>,
    attrs: ExecutionFailAttrs,
}

impl FromDeriveInput for ExecutionFail {
    fn from_derive_input(input: &DeriveInput) -> Result<Self, darling::Error> {
        let attrs = ExecutionFailAttrs::try_from(input.attrs.as_ref())?;
        let data = match &input.data {
            Data::Enum(enum_data) => enum_data,
            _ => {
                let msg = "`ExecutionFail` can only be implemented for enums";
                return Err(darling::Error::unsupported_shape(msg));
            }
        };
        let variants = data
            .variants
            .iter()
            .map(ParsedVariant::from_variant)
            .collect::<Vec<_>>();
        if variants.is_empty() {
            return Err(darling::Error::too_few_items(1));
        }

        Ok(Self {
            name: input.ident.clone(),
            variants,
            attrs,
        })
    }
}

impl ExecutionFail {
    fn implement_display(&self) -> impl ToTokens {
        let name = &self.name;
        let match_arms = self.variants.iter().map(|variant| {
            let ident = &variant.ident;
            let comment = &variant.comment;
            quote! { #name::#ident => f.write_str(#comment), }
        });

        quote! {
            impl std::fmt::Display for #name {
                fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    match self {
                        #( #match_arms )*
                    }
                }
            }
        }
    }

    fn implement_service_fail(&self) -> impl ToTokens {
        let name = &self.name;
        let kind = Ident::new(&self.attrs.kind, Span::call_site());
        let cr = &self.attrs.cr;
        let match_arms = self.variants.iter().map(|variant| {
            let ident = &variant.ident;
            let id = &variant.id;
            quote!(#name::#ident => #cr::runtime::ErrorKind::#kind { code: #id },)
        });

        quote! {
            impl #cr::runtime::ExecutionFail for #name {
                fn kind(&self) -> #cr::runtime::ErrorKind {
                    match self {
                        #( #match_arms )*
                    }
                }

                fn description(&self) -> String {
                    self.to_string()
                }
            }
        }
    }

    fn implement_into_execution_error(&self) -> impl ToTokens {
        let name = &self.name;
        let cr = &self.attrs.cr;
        let module = quote!(#cr::runtime);
        quote! {
            impl From<#name> for #module::ExecutionError {
                fn from(inner: #name) -> Self {
                    let kind = #module::ExecutionFail::kind(&inner);
                    let description = #module::ExecutionFail::description(&inner);
                    #module::ExecutionError::new(kind, description)
                }
            }
        }
    }
}

impl ToTokens for ExecutionFail {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let display_impl = self.implement_display();
        let service_fail_impl = self.implement_service_fail();
        let into_execution_error_impl = self.implement_into_execution_error();

        tokens.extend(quote! {
            #display_impl
            #service_fail_impl
            #into_execution_error_impl
        })
    }
}

pub fn impl_execution_fail(input: TokenStream) -> TokenStream {
    let input = ExecutionFail::from_derive_input(&syn::parse(input).unwrap())
        .unwrap_or_else(|e| panic!("ExecutionFail: {}", e));
    let tokens = quote!(#input);
    tokens.into()
}
