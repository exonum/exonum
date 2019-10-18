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

use proc_macro::TokenStream;
use syn::{Attribute, NestedMeta};

/// ProtobufConvert derive macro.
///
/// Attributes:
///
/// ## Required
///
/// * `#[protobuf_convert(source = "path")]`
///
/// ```ignore
/// #[derive(Clone, Debug, BinaryValue, ObjectHash, ProtobufConvert)]
/// #[protobuf_convert(source = "proto::Wallet")]
/// pub struct Wallet {
///     /// `PublicKey` of the wallet.
///     pub pub_key: PublicKey,
///     /// Current balance of the wallet.
///     pub balance: u64,
/// }
///
/// let wallet = Wallet::new();
/// let serialized_wallet = wallet.to_pb();
///
/// let deserialized_wallet = ProtobufConvert::from_pb(serialized_wallet).unwrap();
/// assert_eq!(wallet, deserialized_wallet);
/// ```
///
/// Corresponding proto file:
/// ```text
/// message Wallet {
///  // Public key of the wallet owner.
///  exonum.crypto.PublicKey pub_key = 1;
///  // Current balance.
///  uint64 balance = 2;
/// }
/// ```
///
/// This macro can also be applied to enums. In proto files enums are represented
/// by `oneof` field. You can specify `oneof` field name, default is "message".
/// ```ignore
/// #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ProtobufConvert)]
/// #[protobuf_convert(source = "schema::runtime::ConfigChange", oneof_field = "message")]
/// pub enum ConfigChange {
///    /// New consensus config.
///    Consensus(ConsensusConfig),
///    /// New service instance config.
///    Service(ServiceConfig),
/// }
/// ```
///
/// Corresponding proto file:
/// ```test
/// message ConfigChange {
///  oneof message {
///    // New consensus config.
///    exonum.Config consensus = 1;
///    // New service instance config.
///    ServiceConfig service = 2;
///  }
/// }
/// ```
///
/// Path is the name of the corresponding protobuf generated struct.
///
/// * `#[protobuf_convert(source = "path", serde_pb_convert)]`
///
/// Implement `serde::{Serialize, Deserialize}` using structs that were generated with
/// protobuf.
/// For example, it should be used if you want json representation of your struct
/// to be compatible with protobuf representation (including proper nesting of fields).
/// For example, struct with `exonum::crypto::Hash` with this
/// (de)serializer will be represented as
/// ```text
/// StructName {
///     "hash": {
///         "data": [1, 2, ...]
///     },
///     // ...
/// }
/// // With default (de)serializer.
/// StructName {
///     "hash": "12af..." // HEX
///     // ...
/// }
/// ```
#[proc_macro_derive(ProtobufConvert, attributes(protobuf_convert))]
pub fn protobuf_convert(input: TokenStream) -> TokenStream {
    pb_convert::implement_protobuf_convert(input)
}

pub(crate) fn find_protobuf_convert_meta(args: &[Attribute]) -> Option<NestedMeta> {
    args.as_ref()
        .iter()
        .filter_map(|a| a.parse_meta().ok())
        .find(|m| m.path().is_ident("protobuf_convert"))
        .map(NestedMeta::from)
}
