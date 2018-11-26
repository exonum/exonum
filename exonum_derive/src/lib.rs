// Copyright 2018 The Exonum Team
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
extern crate proc_macro2;

extern crate syn;
#[macro_use]
extern crate quote;

mod pb_convert;
mod tx_set;

use proc_macro::TokenStream;
use syn::{Attribute, Lit, Meta, MetaList, MetaNameValue, NestedMeta, Path};

use std::env;

/// Exonum derive attribute names, used as `#[exonum( ATTRIBUTE_NAME = .. )]`
const ROOT_PATH_ATTRIBUTE: &str = "root";
const PB_CONVERT_ATTRIBUTE: &str = "pb";

/// Derives `ProtobufConvert` trait.
/// Attributes:
/// `#[exonum( pb = "path" )]`
/// Required. `path` is name of the corresponding protobuf struct(generated from .proto file)
/// `#[exonum( root = "path" )]`
/// Optional. `path` is prefix of the exonum crate(usually "crate" or "exonum")
#[proc_macro_derive(ProtobufConvert, attributes(exonum))]
pub fn generate_protobuf_convert(input: TokenStream) -> TokenStream {
    pb_convert::implement_protobuf_convert(input)
}

/// Derives `TransactionSet` trait for selected enum,
/// enum should be set of variants with transactions inside.
///
/// Also implements:
/// Conversion from Transaction types into this enum.
/// Conversion from Transaction types, enum into `ServiceTransaction`.
/// Conversion from enum into `Box<dyn Transaction>`.
///
/// Attributes:
/// `#[exonum( root = "path" )]`
/// Optional. `path` is prefix of the exonum crate(usually "crate" or "exonum")
#[proc_macro_derive(TransactionSet, attributes(exonum))]
pub fn transaction_set_derive(input: TokenStream) -> TokenStream {
    tx_set::implement_transaction_set(input)
}

/// Exonum types should be imported with `crate::` prefix if inside crate
/// or with `exonum::` when outside.
fn get_exonum_types_prefix(attrs: &[Attribute]) -> impl quote::ToTokens {
    let map_attrs = get_exonum_attributes(attrs);
    let crate_path = map_attrs.into_iter().find_map(|nv| match &nv {
        MetaNameValue {
            lit: Lit::Str(path),
            ident,
            ..
        }
            if ident == ROOT_PATH_ATTRIBUTE =>
        {
            Some(
                path.parse::<Path>()
                    .expect("failed to parse crate path in the attribute"),
            )
        }
        _ => None,
    });

    // If exonum_root_path attribute is defined we use its value
    if let Some(path) = crate_path {
        return quote!(#path);
    }

    // Check cargo env variable to see if we are building inside exonum crate.
    let inside_exonum = env::var("CARGO_PKG_NAME")
        .map(|pkg| pkg == "exonum")
        .unwrap_or(false);

    if inside_exonum {
        quote!(crate)
    } else {
        quote!(exonum)
    }
}

/// Extract attributes in the form of `#[exonum(name = "value")]`
fn get_exonum_attributes(attrs: &[Attribute]) -> Vec<MetaNameValue> {
    let exonum_meta = attrs
        .iter()
        .find_map(|attr| attr.parse_meta().ok().filter(|m| m.name() == "exonum"));

    match exonum_meta {
        Some(Meta::List(MetaList { nested: list, .. })) => list
            .into_iter()
            .filter_map(|n| match n {
                NestedMeta::Meta(Meta::NameValue(named)) => Some(named),
                _ => None,
            }).collect(),
        Some(_) => panic!("`exonum` attribute should contain list of name value pairs"),
        None => vec![],
    }
}
