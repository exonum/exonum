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

#![recursion_limit = "256"]

extern crate proc_macro;

mod pb_convert;
mod service_interface;
mod tx_set;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Lit, Meta, MetaList, MetaNameValue, NestedMeta, Path};

// Exonum derive attribute names, used as
// `#[exonum( [ ATTRIBUTE_NAME = ATTRIBUTE_VALUE or ATTRIBUTE_NAME ],* )]`
const CRATE_PATH_ATTRIBUTE: &str = "crate";
const PB_CONVERT_ATTRIBUTE: &str = "pb";
const SERDE_PB_CONVERT_ATTRIBUTE: &str = "serde_pb_convert";

/// Derives `ProtobufConvert` trait.
///
/// Attributes:
///
/// * `#[exonum( pb = "path" )]`
/// Required. `path` is the name of the corresponding protobuf generated struct.
///
/// * `#[exonum( crate = "path" )]`
/// Optional. `path` is prefix of the `exonum` crate(usually "crate" or "exonum").
///
/// * `#[exonum( serde_pb_convert )]`
/// Optional. Implements `serde::{Serialize, Deserialize}` using structs that were generated with
/// protobuf.
/// For example, it should be used if you want json representation of your struct
/// to be compatible with protobuf representation (including proper nesting of fields).
/// ```text
/// // For example, struct with `exonum::crypto::Hash` with this
/// // (de)serializer will be represented as
/// StructName {
///     "hash": {
///         data: [1, 2, ...]
///     },
///     // ...
/// }
///
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

/// Derives `TransactionSet` trait for selected enum,
/// enum should have transactions as variants.
///
/// Also implements:
///
/// * Conversion from Transaction types into this enum.
/// * Conversion from Transaction types and this enum into `ServiceTransaction`.
/// * Conversion from this enum into `Box<dyn Transaction>`.
///
/// Attributes:
///
/// * `#[exonum( crate = "path" )]`
/// Optional. `path` is a prefix of types from the `exonum` crate (usually "crate" or "exonum").
#[proc_macro_derive(TransactionSet, attributes(exonum))]
pub fn transaction_set_derive(input: TokenStream) -> TokenStream {
    tx_set::implement_transaction_set(input)
}

#[proc_macro_attribute]
pub fn service_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    service_interface::impl_service_interface(attr, item)
}

/// Exonum types should be imported with `crate::` prefix if inside crate
/// or with `exonum::` when outside.
fn get_exonum_types_prefix(attrs: &[Meta]) -> impl quote::ToTokens {
    let map_attrs = get_exonum_name_value_attributes(attrs);
    let crate_path = map_attrs.into_iter().find_map(|nv| match &nv {
        MetaNameValue {
            lit: Lit::Str(path),
            ident,
            ..
        } if ident == CRATE_PATH_ATTRIBUTE => Some(
            path.parse::<Path>()
                .expect("failed to parse crate path in the attribute"),
        ),
        _ => None,
    });

    if let Some(path) = crate_path {
        quote!(#path)
    } else {
        quote!(exonum)
    }
}

/// Extract attributes in the form of `#[exonum(name = "value")]`
fn get_exonum_attributes(exonum_meta: Option<Meta>) -> Vec<Meta> {
    match exonum_meta {
        Some(Meta::List(MetaList { nested: list, .. })) => list
            .into_iter()
            .filter_map(|n| match n {
                NestedMeta::Meta(meta) => Some(meta),
                _ => None,
            })
            .collect(),
        Some(_) => panic!("`exonum` attribute should contain list of name value pairs"),
        None => vec![],
    }
}

fn get_exonum_name_value_attributes(meta_attrs: &[Meta]) -> Vec<MetaNameValue> {
    let exonum_meta = meta_attrs.iter().find(|m| m.name() == "exonum").cloned();

    get_exonum_attributes(exonum_meta)
        .into_iter()
        .filter_map(|meta| match meta {
            Meta::NameValue(name_value) => Some(name_value),
            _ => None,
        })
        .collect()
}

fn find_exonum_word_attribute(meta_attrs: &[Meta], ident_name: &str) -> bool {
    let exonum_meta = meta_attrs.iter().find(|m| m.name() == "exonum").cloned();

    get_exonum_attributes(exonum_meta)
        .iter()
        .any(|meta| match meta {
            Meta::Word(ident) if ident == ident_name => true,
            _ => false,
        })
}
