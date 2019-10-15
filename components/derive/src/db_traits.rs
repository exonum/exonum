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
use proc_macro2::{Ident, Span};
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use darling::{FromDeriveInput, FromMeta};
use syn::DeriveInput;

#[derive(Debug, FromDeriveInput)]
struct DbObject {
    ident: Ident,
}

#[derive(Debug, FromDeriveInput)]
struct DbObjectHash {
    ident: Ident,
}

impl DbObjectHash {
    pub fn implement_object_hash(&self) -> impl ToTokens {
        let name = &self.ident;

        quote! {
            impl exonum_merkledb::ObjectHash for #name {
                fn object_hash(&self) -> exonum_crypto::Hash {
                    use exonum_merkledb::BinaryValue;
                    let v = self.to_bytes();
                    exonum_crypto::hash(&v)
                }
            }
        }
    }
}

impl DbObject {
    pub fn implement_binary_value(&self) -> impl ToTokens {
        let name = &self.ident;
        let cr = "";

        quote! {
            // This trait assumes that we work with trusted data so we can unwrap here.
            impl exonum_merkledb::BinaryValue for #name {
                fn to_bytes(&self) -> Vec<u8> {
                    self.to_pb().write_to_bytes().expect(
                        concat!("Failed to serialize in BinaryValue for ", stringify!(#name))
                    )
                }

                fn from_bytes(value: std::borrow::Cow<[u8]>) -> Result<Self, failure::Error> {
                    let mut block = <Self as exonum_proto::ProtobufConvert>::ProtoStruct::new();
                    block.merge_from_bytes(value.as_ref())?;
                    exonum_proto::ProtobufConvert::from_pb(block)
                }
            }
        }
    }
}

impl ToTokens for DbObject {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.ident;

        let mod_name = Ident::new(
            &format!("binary_value_impl_{}", self.ident),
            Span::call_site(),
        );

        let binary_value = self.implement_binary_value();
        let expanded = quote! {
            mod #mod_name {
                use super::*;

                use protobuf::Message as _ProtobufMessage;
                use exonum_proto::ProtobufConvert;

                #binary_value
            }
        };

        tokens.extend(expanded);
    }
}

impl ToTokens for DbObjectHash {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.ident;

        let object_hash = self.implement_object_hash();
        let expanded = quote! { #object_hash };

        tokens.extend(expanded);
    }
}

pub fn binary_value(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let db_object = DbObject::from_derive_input(&input)
        .unwrap_or_else(|e| panic!("BinaryValue: {}", e));
    let tokens = quote! {#db_object};
    tokens.into()
}

pub fn object_hash(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let db_object = DbObjectHash::from_derive_input(&input)
        .unwrap_or_else(|e| panic!("ObjectHash: {}", e));
    let tokens = quote! {#db_object};
    tokens.into()
}

