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
mod tx_set;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Attribute, Lit, Meta, MetaList, MetaNameValue, NestedMeta, Path};

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

/// Derives `TransactionSet` trait for an enum. The enum should have transactions as variants.
///
/// Also implements:
///
/// - Conversion from variant types into this enum.
/// - Conversion from this enum into `ServiceTransaction`.
/// - Conversion from variants into `ServiceTransaction` (opt-out; see [Attributes](#attributes)).
/// - Conversion from this enum into `Box<dyn Transaction>`.
///
/// # Attributes
///
/// ## Crate specification
///
/// ```text
/// #[exonum(crate = "path")]
/// ```
///
/// Optional. `path` is a prefix of types from the `exonum` crate (usually `"crate"`
/// or `"exonum"`).
///
/// ## Conversions for variants
///
/// ```text
/// #[exonum(convert_variants = value)]
/// ```
///
/// Optional. `value` is `bool` or string convertible to `bool` determining if the macro
/// should derive conversions into `ServiceTransaction` for enum variants.
/// Switching derivation off is useful (or even necessary)
/// if the same variant is used in several `TransactionSet`s, or is external to the crate
/// where `TransactionSet` is defined.
///
/// ## Message IDs for variants
///
/// ```text
/// #[exonum(message_id = value)]
/// ```
///
/// Optional; specified on variants. `value` is a `u16` value, or a string convertible to a `u16`
/// value. Assignment of IDs acts like discriminants in Rust enums:
///
/// - By default, `message_id`s are assigned from zero and increase by 1 for each variant.
/// - If a `message_id` is specified for a variant, but not specified on the following variants,
///   the `message_id` on the following variants is produced by increasing the last explicit
///   value.
///
/// # Examples
///
/// ```
/// # use exonum::blockchain::{ExecutionResult, Transaction, TransactionContext};
/// # use exonum_derive::*;
/// use serde_derive::*;
/// # mod proto {
/// #    pub type Issue = exonum::proto::Hash;
/// #    pub type Transfer = exonum::proto::Hash;
/// # }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, ProtobufConvert)]
/// #[exonum(pb = "proto::Issue")]
/// pub struct Issue { /* ... */ }
/// impl Transaction for Issue {
///     // ...
/// #   fn execute(&self, context: TransactionContext) -> ExecutionResult {
/// #       Ok(())
/// #   }
/// }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, ProtobufConvert)]
/// #[exonum(pb = "proto::Transfer")]
/// pub struct Transfer { /* ... */ }
/// impl Transaction for Transfer {
///     // ...
/// #   fn execute(&self, context: TransactionContext) -> ExecutionResult {
/// #         Ok(())
/// #   }
/// }
///
/// /// Transactions of some service.
/// #[derive(Debug, Clone, Serialize, Deserialize, TransactionSet)]
/// #[exonum(convert_variants = false)]
/// pub enum Transactions {
///     /// Issuance transaction.
///     Issue(Issue),
///     /// Transfer transaction.
///     Transfer(Transfer),
/// }
///
/// /// Transactions of some other service (may be defined in other crate).
/// #[derive(Debug, Clone, Serialize, Deserialize, TransactionSet)]
/// #[exonum(convert_variants = false)]
/// pub enum OtherTransactions {
///     #[exonum(message_id = 5)]
///     Transfer(Transfer),
///     // Other transactions...
/// }
/// # fn main() {}
/// ```
///
/// It is possible to box variants in order to reduce their stack size:
///
/// ```
/// # use exonum::blockchain::{ExecutionResult, Transaction, TransactionContext};
/// # use exonum_derive::*;
/// use serde_derive::*;
/// # mod proto {
/// #    pub type Issue = exonum::proto::Hash;
/// #    pub type Transfer = exonum::proto::Hash;
/// # }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, ProtobufConvert)]
/// #[exonum(pb = "proto::Issue")]
/// pub struct Issue { /* a lot of fields */ }
/// # impl Transaction for Issue {
/// #   fn execute(&self, context: TransactionContext) -> ExecutionResult {
/// #         Ok(())
/// #   }
/// # }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, TransactionSet)]
/// pub enum Transactions {
///     Issue(Box<Issue>),
///     // other variants...
/// }
///
/// # fn main () {
/// let tx: Transactions = Issue { /* ... */ }.into();
/// # }
/// ```
#[proc_macro_derive(TransactionSet, attributes(exonum))]
pub fn transaction_set_derive(input: TokenStream) -> TokenStream {
    tx_set::implement_transaction_set(input)
}

/// Exonum types should be imported with `crate::` prefix if inside crate
/// or with `exonum::` when outside.
fn get_exonum_types_prefix(attrs: &[Attribute]) -> impl quote::ToTokens {
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
fn get_exonum_attributes(attrs: &[Attribute]) -> Vec<Meta> {
    let exonum_meta = attrs
        .iter()
        .find_map(|attr| attr.parse_meta().ok().filter(|m| m.name() == "exonum"));

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

fn get_exonum_name_value_attributes(attrs: &[Attribute]) -> Vec<MetaNameValue> {
    get_exonum_attributes(attrs)
        .into_iter()
        .filter_map(|meta| match meta {
            Meta::NameValue(name_value) => Some(name_value),
            _ => None,
        })
        .collect()
}

fn find_exonum_word_attribute(attrs: &[Attribute], ident_name: &str) -> bool {
    get_exonum_attributes(attrs).iter().any(|meta| match meta {
        Meta::Word(ident) if ident == ident_name => true,
        _ => false,
    })
}
