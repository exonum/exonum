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

//! This crate provides macros for deriving some useful methods and traits for the exonum services.

#![recursion_limit = "128"]
#![deny(unsafe_code, bare_trait_objects)]
#![warn(missing_docs, missing_debug_implementations)]

extern crate proc_macro;

mod pb_convert;

use darling::FromMeta;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{Attribute, NestedMeta};

/// Derive `ProtobufConvert` trait.
///
/// Attributes:
///
/// ## Required
///
/// * `#[exonum(pb = "path")]`
///
/// Path is the name of the corresponding protobuf generated struct.
///
/// ## Optional
///
/// * `#[exonum(crate = "path")]`
///
/// Prefix of the `exonum` crate(usually "crate" or "exonum").
///
/// * `#[exonum(serde_pb_convert)]`
///
/// Implement `serde::{Serialize, Deserialize}` using structs that were generated with
/// protobuf.
/// For example, it should be used if you want json representation of your struct
/// to be compatible with protobuf representation (including proper nesting of fields).
/// ```text
/// For example, struct with `exonum::crypto::Hash` with this
/// (de)serializer will be represented as
/// StructName {
///     "hash": {
///         data: [1, 2, ...]
///     },
///     // ...
/// }
/// // With default (de)serializer.
/// StructName {
///     "hash": "12af..." // HEX
///     // ...
/// }
/// ```
#[proc_macro_derive(ProtobufConvert, attributes(exonum))]
pub fn generate_protobuf_convert(input: TokenStream) -> TokenStream {
    pb_convert::implement_protobuf_convert(input)
}

pub(crate) fn find_exonum_meta(args: &[Attribute]) -> Option<NestedMeta> {
    args.as_ref()
        .iter()
        .filter_map(|a| a.parse_meta().ok())
        .find(|m| m.path().is_ident("exonum"))
        .map(NestedMeta::from)
}

#[derive(Debug, FromMeta, PartialEq, Eq)]
#[darling(default)]
struct CratePath(syn::Path);

impl Default for CratePath {
    fn default() -> Self {
        Self(syn::parse_str("exonum").unwrap())
    }
}

impl ToTokens for CratePath {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}
