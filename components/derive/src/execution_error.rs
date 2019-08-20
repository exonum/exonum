// Copyright 2019 The Exonum Team
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
use proc_macro2::Ident;
use quote::{quote, ToTokens};
use syn::{Attribute, Data, DeriveInput, Expr, MetaNameValue, Path, Variant};

use std::convert::TryFrom;

use super::find_exonum_meta;

#[derive(Debug, FromMeta)]
#[darling(default)]
struct ExecutionErrorAttrs {
    #[darling(rename = "crate")]
    cr: Path,
    kind: Path,
}

impl Default for ExecutionErrorAttrs {
    fn default() -> Self {
        Self {
            cr: syn::parse_str("exonum").unwrap(),
            kind: syn::parse_str("service").unwrap(),
        }
    }
}

impl TryFrom<&[Attribute]> for ExecutionErrorAttrs {
    type Error = darling::Error;

    fn try_from(args: &[Attribute]) -> Result<Self, Self::Error> {
        find_exonum_meta(args)
            .map(|meta| Self::from_nested_meta(&meta))
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
            "IntoExecutionError: Each enum variant should not have fields inside."
        );
        let discriminant = variant
            .discriminant
            .clone()
            .expect("IntoExecutionError: Each enum variant should have an explicit discriminant declaration")
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
                    attr.interpret_meta()
                } else {
                    None
                }
            })
            .filter_map(|attr| {
                use crate::Lit::*;
                use crate::Meta::*;
                if let NameValue(MetaNameValue {
                    ident, lit: Str(s), ..
                }) = attr
                {
                    if ident != "doc" {
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
struct IntoExecutionError {
    name: Ident,
    variants: Vec<ParsedVariant>,
    attrs: ExecutionErrorAttrs,
}

impl FromDeriveInput for IntoExecutionError {
    fn from_derive_input(input: &DeriveInput) -> Result<Self, darling::Error> {
        let attrs = ExecutionErrorAttrs::try_from(input.attrs.as_ref())?;
        let data = match &input.data {
            Data::Enum(enum_data) => enum_data,
            _ => return Err(darling::Error::unsupported_shape("Only for enums.")),
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

impl IntoExecutionError {
    fn implement_display(&self) -> impl ToTokens {
        let name = &self.name;
        let match_arms = self.variants.iter().map(|variant| {
            let ident = &variant.ident;
            let comment = &variant.comment;
            quote! { #name::#ident => f.write_str(#comment), }
        });

        quote! {
            impl ::std::fmt::Display for #name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                    match self {
                        #( #match_arms )*
                    }
                }
            }
        }
    }

    fn implement_into_execution_error(&self) -> impl ToTokens {
        let name = &self.name;
        let cr = &self.attrs.cr;
        let kind = &self.attrs.kind;
        let match_arms = self.variants.iter().map(|variant| {
            let ident = &variant.ident;
            let id = &variant.id;
            quote! { #name::#ident => {
                let kind = #cr::runtime::error::ErrorKind::#kind(#id);
                #cr::runtime::error::ExecutionError::new(kind, inner.to_string())
            } }
        });

        quote! {
            impl From<#name> for #cr::runtime::error::ExecutionError {
                fn from(inner: #name) -> Self {
                    match inner {
                        #( #match_arms )*
                    }
                }
            }
        }
    }

    fn implement_into_error_kind(&self) -> impl ToTokens {
        let name = &self.name;
        let cr = &self.attrs.cr;
        let kind = &self.attrs.kind;
        let match_arms = self.variants.iter().map(|variant| {
            let ident = &variant.ident;
            let id = &variant.id;
            quote! { #name::#ident => #cr::runtime::error::ErrorKind::#kind(#id), }
        });

        quote! {
            impl From<#name> for #cr::runtime::error::ErrorKind {
                fn from(inner: #name) -> Self {
                    match inner {
                        #( #match_arms )*
                    }
                }
            }
        }
    }
}

impl ToTokens for IntoExecutionError {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let display_impl = self.implement_display();
        let into_execution_error_impl = self.implement_into_execution_error();
        let into_error_kind_impl = self.implement_into_error_kind();

        tokens.extend(quote! {
            #display_impl
            #into_execution_error_impl
            #into_error_kind_impl
        })
    }
}

pub fn implement_execution_error(input: TokenStream) -> TokenStream {
    let input = IntoExecutionError::from_derive_input(&syn::parse(input).unwrap())
        .unwrap_or_else(|e| panic!("IntoExecutionError: {}", e));
    let tokens = quote! {#input};
    tokens.into()
}
