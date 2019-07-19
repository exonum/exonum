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
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use syn::{Attribute, Data, DataStruct, DeriveInput, NestedMeta, Path};

use std::convert::TryFrom;

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
        args.as_ref()
            .iter()
            .filter_map(|a| a.parse_meta().ok())
            .find(|m| m.name() == "exonum")
            .map(|meta| Self::from_nested_meta(&NestedMeta::from(meta)))
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
    oneof_name: String,
}

impl Default for ProtobufConvertEnumAttrs {
    fn default() -> Self {
        Self {
            cr: syn::parse_str("exonum").unwrap(),
            pb: None,
            oneof_name: "message".to_owned(),
            serde_pb_convert: false,
        }
    }
}

impl TryFrom<&[Attribute]> for ProtobufConvertEnumAttrs {
    type Error = darling::Error;

    fn try_from(args: &[Attribute]) -> Result<Self, Self::Error> {
        args.as_ref()
            .iter()
            .filter_map(|a| a.parse_meta().ok())
            .find(|m| m.name() == "exonum")
            .map(|meta| Self::from_nested_meta(&NestedMeta::from(meta)))
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
        let cr = &self.attrs.cr;
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
                let mut msg = Self::ProtoStruct::new();
                #( msg.#setters(ProtobufConvert::to_pb(&self.#fields).into()); )*
                msg
            }
        };

        let expanded = quote! {
            impl ProtobufConvert for #name {
                type ProtoStruct = #pb_name;

                fn from_pb(mut pb: Self::ProtoStruct) -> std::result::Result<Self, _FailureError> {
                    #from_pb_impl
                }

                fn to_pb(&self) -> Self::ProtoStruct {
                    #to_pb_impl
                }
            }

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
        };
        println!("{}", expanded);

        tokens.extend(expanded);
    }
}

#[derive(Debug)]
struct ProtobufConvertEnum {
    name: Ident,
    variants: Vec<Ident>,
    attrs: ProtobufConvertEnumAttrs,
}

impl ToTokens for ProtobufConvertEnum {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let expanded = quote! {};
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
            Data::Enum(data) => unimplemented!(),
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
        let serde_traits = if self.serde_needed() {
            let serde = self.implement_serde_protobuf_convert();
            quote! { #serde }
        } else {
            quote! {}
        };

        let expanded = quote! {
            mod #mod_name {
                extern crate protobuf as _protobuf_crate;
                extern crate failure as _failure;

                use super::*;

                use self::_protobuf_crate::Message as _ProtobufMessage;
                use self::_failure::{bail, Error as _FailureError};
                use #cr::proto::ProtobufConvert;

                #protobuf_convert
                #merkledb_traits
                #serde_traits
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
