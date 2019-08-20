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
use heck::SnakeCase;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use syn::{Attribute, Data, DataEnum, DataStruct, DeriveInput, Path};

use std::convert::TryFrom;

use super::find_exonum_meta;

#[derive(Debug, FromMeta)]
#[darling(default)]
struct ProtobufConvertStructAttrs {
    #[darling(rename = "crate")]
    cr: Path,
    pb: Option<Path>,
    serde_pb_convert: bool,
}

impl Default for ProtobufConvertStructAttrs {
    fn default() -> Self {
        Self {
            cr: syn::parse_str("exonum").unwrap(),
            pb: None,
            serde_pb_convert: false,
        }
    }
}

impl TryFrom<&[Attribute]> for ProtobufConvertStructAttrs {
    type Error = darling::Error;

    fn try_from(args: &[Attribute]) -> Result<Self, Self::Error> {
        find_exonum_meta(args)
            .map(|meta| Self::from_nested_meta(&meta))
            .unwrap_or_else(|| Ok(Self::default()))
    }
}

#[derive(Debug, FromMeta)]
#[darling(default)]
struct ProtobufConvertEnumAttrs {
    #[darling(rename = "crate")]
    cr: Path,
    pb: Option<Path>,
    serde_pb_convert: bool,
    oneof_field: Ident,
}

impl Default for ProtobufConvertEnumAttrs {
    fn default() -> Self {
        Self {
            cr: syn::parse_str("exonum").unwrap(),
            pb: None,
            oneof_field: syn::parse_str("message").unwrap(),
            serde_pb_convert: false,
        }
    }
}

impl TryFrom<&[Attribute]> for ProtobufConvertEnumAttrs {
    type Error = darling::Error;

    fn try_from(args: &[Attribute]) -> Result<Self, Self::Error> {
        find_exonum_meta(args)
            .map(|meta| Self::from_nested_meta(&meta))
            .unwrap_or_else(|| Ok(Self::default()))
    }
}

#[derive(Debug)]
struct ProtobufConvertStruct {
    name: Ident,
    fields: Vec<Ident>,
    attrs: ProtobufConvertStructAttrs,
}

impl ProtobufConvertStruct {
    fn from_derive_input(
        name: Ident,
        data: &DataStruct,
        attrs: &[Attribute],
    ) -> Result<Self, darling::Error> {
        let attrs = ProtobufConvertStructAttrs::try_from(attrs)?;
        let fields = data
            .fields
            .iter()
            .map(|f| f.ident.clone().unwrap())
            .collect();

        Ok(Self {
            name,
            attrs,
            fields,
        })
    }
}

impl ToTokens for ProtobufConvertStruct {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let pb_name = &self.attrs.pb;
        let from_pb_impl = {
            let getters = self
                .fields
                .iter()
                .map(|i| Ident::new(&format!("get_{}", i), Span::call_site()));
            let fields = self.fields.to_vec();

            quote! {
                let inner = Self {
                    #( #fields: ProtobufConvert::from_pb(pb.#getters().to_owned())?, )*
                };
                Ok(inner)
            }
        };
        let to_pb_impl = {
            let setters = self
                .fields
                .iter()
                .map(|i| Ident::new(&format!("set_{}", i), Span::call_site()));
            let fields = self.fields.to_vec();

            quote! {
                let mut msg = Self::ProtoStruct::default();
                #( msg.#setters(ProtobufConvert::to_pb(&self.#fields).into()); )*
                msg
            }
        };

        let expanded = quote! {
            impl ProtobufConvert for #name {
                type ProtoStruct = #pb_name;

                fn from_pb(pb: Self::ProtoStruct) -> std::result::Result<Self, failure::Error> {
                    #from_pb_impl
                }

                fn to_pb(&self) -> Self::ProtoStruct {
                    #to_pb_impl
                }
            }
        };
        tokens.extend(expanded);
    }
}

#[derive(Debug)]
struct ProtobufConvertEnum {
    name: Ident,
    variants: Vec<Ident>,
    attrs: ProtobufConvertEnumAttrs,
}

impl ProtobufConvertEnum {
    fn from_derive_input(
        name: Ident,
        data: &DataEnum,
        attrs: &[Attribute],
    ) -> Result<Self, darling::Error> {
        let attrs = ProtobufConvertEnumAttrs::try_from(attrs)?;
        let variants = data.variants.iter().map(|v| v.ident.clone()).collect();

        Ok(Self {
            name,
            attrs,
            variants,
        })
    }

    fn impl_protobuf_convert(&self) -> impl ToTokens {
        let pb_oneof_enum = {
            let mut pb = self.attrs.pb.clone().unwrap();
            let oneof = pb.segments.pop().unwrap().value().ident.clone();
            let oneof_enum = Ident::new(
                &format!("{}_oneof_{}", oneof, &self.attrs.oneof_field),
                Span::call_site(),
            );
            quote! { #pb #oneof_enum }
        };
        let name = &self.name;
        let pb_name = &self.attrs.pb;
        let oneof = &self.attrs.oneof_field;

        let from_pb_impl = {
            let match_arms = self.variants.iter().map(|variant| {
                let pb_variant = Ident::new(
                    variant.to_string().to_snake_case().as_ref(),
                    Span::call_site(),
                );
                quote! {
                    Some(#pb_oneof_enum::#pb_variant(pb)) => {
                        #variant::from_pb(pb).map(#name::#variant)
                    }
                }
            });

            quote! {
                match pb.#oneof {
                    #( #match_arms )*
                    None => Err(failure::format_err!("Failed to decode #name from protobuf"))
                }
            }
        };
        let to_pb_impl = {
            let match_arms = self.variants.iter().map(|variant| {
                let pb_variant = variant.to_string().to_snake_case();
                let setter = Ident::new(&format!("set_{}", pb_variant), Span::call_site());
                quote! {
                    #name::#variant(msg) => inner.#setter(msg.to_pb()),
                }
            });

            quote! {
                let mut inner = Self::ProtoStruct::new();
                match self {
                    #( #match_arms )*
                }
                inner
            }
        };

        quote! {
            impl ProtobufConvert for #name {
                type ProtoStruct = #pb_name;

                fn from_pb(mut pb: Self::ProtoStruct) -> std::result::Result<Self, failure::Error> {
                    #from_pb_impl
                }

                fn to_pb(&self) -> Self::ProtoStruct {
                    #to_pb_impl
                }
            }
        }
    }

    fn impl_enum_conversions(&self) -> impl ToTokens {
        let name = &self.name;
        let conversions = self.variants.iter().map(|variant| {
            quote! {
                impl From<#variant> for #name {
                    fn from(variant: #variant) -> Self {
                        #name::#variant(variant)
                    }
                }

                impl std::convert::TryFrom<#name> for #variant {
                    type Error = failure::Error;

                    fn try_from(msg: #name) -> Result<Self, Self::Error> {
                        if let #name::#variant(inner) = msg {
                            Ok(inner)
                        } else {
                            Err(failure::format_err!(
                                "Expected #variant variant, but got {:?}",
                                msg
                            ))
                        }
                    }
                }
            }
        });

        quote! {
            #( #conversions )*
        }
    }
}

impl ToTokens for ProtobufConvertEnum {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let pb_convert = self.impl_protobuf_convert();
        let conversions = self.impl_enum_conversions();

        let expanded = quote! {
            #pb_convert
            #conversions
        };
        tokens.extend(expanded)
    }
}

#[derive(Debug)]
enum ProtobufConvert {
    Enum(ProtobufConvertEnum),
    Struct(ProtobufConvertStruct),
}

impl FromDeriveInput for ProtobufConvert {
    fn from_derive_input(input: &DeriveInput) -> Result<Self, darling::Error> {
        match &input.data {
            Data::Struct(data) => Ok(ProtobufConvert::Struct(
                ProtobufConvertStruct::from_derive_input(
                    input.ident.clone(),
                    data,
                    input.attrs.as_ref(),
                )?,
            )),
            Data::Enum(data) => Ok(ProtobufConvert::Enum(
                ProtobufConvertEnum::from_derive_input(
                    input.ident.clone(),
                    data,
                    input.attrs.as_ref(),
                )?,
            )),
            _ => Err(darling::Error::unsupported_shape(
                "Only for enums and structs.",
            )),
        }
    }
}

impl ProtobufConvert {
    fn name(&self) -> &Ident {
        match self {
            ProtobufConvert::Enum(inner) => &inner.name,
            ProtobufConvert::Struct(inner) => &inner.name,
        }
    }

    fn cr(&self) -> &Path {
        match self {
            ProtobufConvert::Enum(inner) => &inner.attrs.cr,
            ProtobufConvert::Struct(inner) => &inner.attrs.cr,
        }
    }

    fn serde_needed(&self) -> bool {
        match self {
            ProtobufConvert::Enum(inner) => inner.attrs.serde_pb_convert,
            ProtobufConvert::Struct(inner) => inner.attrs.serde_pb_convert,
        }
    }

    fn implement_merkledb_traits(&self) -> impl ToTokens {
        let name = self.name();
        let cr = self.cr();

        quote! {
            impl exonum_merkledb::ObjectHash for #name {
                fn object_hash(&self) -> #cr::crypto::Hash {
                    let v = self.to_pb().write_to_bytes().unwrap();
                    #cr::crypto::hash(&v)
                }
            }

            // This trait assumes that we work with trusted data so we can unwrap here.
            impl exonum_merkledb::BinaryValue for #name {
                fn to_bytes(&self) -> Vec<u8> {
                    self.to_pb().write_to_bytes().expect(
                        concat!("Failed to serialize in BinaryValue for ", stringify!(#name))
                    )
                }

                fn from_bytes(value: std::borrow::Cow<[u8]>) -> Result<Self, failure::Error> {
                    let mut block = <Self as ProtobufConvert>::ProtoStruct::new();
                    block.merge_from_bytes(value.as_ref())?;
                    ProtobufConvert::from_pb(block)
                }
            }
        }
    }

    fn implement_serde_protobuf_convert(&self) -> impl ToTokens {
        let name = self.name();
        quote! {
            impl serde::Serialize for #name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: serde::Serializer,
                {
                    self.to_pb().serialize(serializer)
                }
            }

            impl<'de> serde::Deserialize<'de> for #name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    let pb = <#name as ProtobufConvert>::ProtoStruct::deserialize(deserializer)?;
                    ProtobufConvert::from_pb(pb).map_err(serde::de::Error::custom)
                }
            }
        }
    }

    fn implement_protobuf_convert(&self) -> impl ToTokens {
        match self {
            ProtobufConvert::Enum(data) => quote! { #data },
            ProtobufConvert::Struct(data) => quote! { #data },
        }
    }

    fn implement_any_conversion(&self) -> impl ToTokens {
        let name = self.name();
        let cr = self.cr();

        quote! {
            impl From<#name> for #cr::proto::Any {
                fn from(v: #name) -> Self {
                    Self::new(v)
                }
            }

            impl std::convert::TryFrom<#cr::proto::Any> for #name {
                type Error = failure::Error;

                fn try_from(v: #cr::proto::Any) -> Result<Self, Self::Error> {
                    v.try_into()
                }
            }
        }
    }
}

impl ToTokens for ProtobufConvert {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mod_name = Ident::new(
            &format!("pb_convert_impl_{}", self.name()),
            Span::call_site(),
        );
        let cr = self.cr();

        let protobuf_convert = self.implement_protobuf_convert();
        let merkledb_traits = self.implement_merkledb_traits();
        let any_conversion = self.implement_any_conversion();
        let serde_traits = if self.serde_needed() {
            let serde = self.implement_serde_protobuf_convert();
            quote! { #serde }
        } else {
            quote! {}
        };

        let expanded = quote! {
            mod #mod_name {
                use super::*;

                use protobuf::Message as _ProtobufMessage;
                use #cr::proto::ProtobufConvert;

                #protobuf_convert
                #merkledb_traits
                #serde_traits
                #any_conversion
            }
        };
        tokens.extend(expanded)
    }
}

pub fn implement_protobuf_convert(input: TokenStream) -> TokenStream {
    let input = ProtobufConvert::from_derive_input(&syn::parse(input).unwrap())
        .unwrap_or_else(|e| panic!("ProtobufConvert: {}", e));
    let tokens = quote! {#input};
    tokens.into()
}
