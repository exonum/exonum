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

mod db_traits;
mod exonum_interface;
mod service_dispatcher;
mod service_factory;
mod service_fail;

use darling::FromMeta;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{Attribute, NestedMeta};

/// Derive `BinaryValue` trait.
/// Target type must implement `ProtobufConvert` trait.
///
/// # Example
/// ```ignore
/// #[derive(Clone, Debug, BinaryValue)]
/// #[protobuf_convert(source = "proto::Wallet")]
/// pub struct Wallet {
///     /// `PublicKey` of the wallet.
///     pub pub_key: PublicKey,
///     /// Current balance of the wallet.
///     pub balance: u64,
/// }
///
/// let wallet = Wallet::new();
/// let bytes = wallet.to_bytes();
/// ```
#[proc_macro_derive(BinaryValue)]
pub fn binary_value(input: TokenStream) -> TokenStream {
    db_traits::impl_binary_value(input)
}

/// Derive `ObjectHash` trait.
/// Target type must implement `BinaryValue` trait.
///
/// # Example
/// ```ignore
/// #[protobuf_convert(source = "proto::Wallet")]
/// #[derive(Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
/// pub struct Wallet {
///     /// `PublicKey` of the wallet.
///     pub pub_key: PublicKey,
///     /// Current balance of the wallet.
///     pub balance: u64,
/// }
///
/// let wallet = Wallet::new();
/// let hash = wallet.object_hash();
/// ```
#[proc_macro_derive(ObjectHash)]
pub fn object_hash(input: TokenStream) -> TokenStream {
    db_traits::impl_object_hash(input)
}

/// Derive `FromAccess` trait.
///
/// This macro can be applied only to `struct`s, each field of which implements `FromAccess`
/// itself (e.g., indexes, `Group`s, or `Lazy` indexes). The macro instantiates each field
/// using the address created by appending a dot `.` and the name of the field or its override
/// (see [below](#rename)) to the root address where the struct is created. For example,
/// if the struct is created at the address `"foo"` and has fields `"list"` and `"map"`, they
/// will be instantiated at addresses `"foo.list"` and `"foo.map"`, respectively.
///
/// The struct must have at least one type param, which will correspond to the `Access` type.
/// The derive logic will determine this param as the first param with `T: Access` bound.
/// If there are no such params, but there is a single type param, it will be used.
///
/// # Container Attributes
///
/// ## `transparent`
///
/// ```text
/// #[from_access(transparent)]`
/// ```
///
/// Switches to the *transparent* layout similarly to `#[repr(transparent)]`
/// or `#[serde(transparent)]`.
/// A struct with the transparent layout must have a single field. The field will be created at
/// the same address as the struct itself (i.e., no suffix will be added).
///
/// ## `schema`
///
/// ```text
/// #[from_access(schema)]
/// ```
///
/// Derives schema-specific interfaces:
///
/// - Constructor `pub fn new(access: T) -> Self` with a generic doc comment. Implemented
///   by `unwrap()`ing the value returned by `FromAccess::from_root`.
///
/// The `schema` param is automatically switched on if the struct name ends with `Schema`.
/// To opt out, use `#[from_access(schema = false)]`.
///
/// # Field Attributes
///
/// ## `rename`
///
/// ```text
/// #[from_access(rename = "name")]
/// ```
///
/// Changes the suffix appended to the address when creating a field. The name should follow
/// conventions for index names.
#[proc_macro_derive(FromAccess, attributes(from_access))]
pub fn from_access(input: TokenStream) -> TokenStream {
    db_traits::impl_from_access(input)
}

/// Derive `ServiceDispatcher` trait.
///
/// # Attributes:
///
/// ## Required
///
/// * `#[service_dispatcher(implements(""path_1", "path_2""))]`
///
/// Path list to the interfaces which have been implemented by the service.
///
/// ## Optional
///
/// * `#[service_dispatcher(crate = "path")]`
///
/// Prefix of the `exonum` crate has two main values - "crate" or "exonum". The default value is "exonum".
#[proc_macro_derive(ServiceDispatcher, attributes(service_dispatcher))]
pub fn service_dispatcher(input: TokenStream) -> TokenStream {
    service_dispatcher::impl_service_dispatcher(input)
}

/// Derive `ServiceFactory` trait.
///
/// # Attributes:
///
/// ## Required
///
/// * `#[service_factory(proto_sources = "path")]`
///
/// Path to the module that was generated by the `exonum_build::protobuf_generate`
/// and contains the original Protobuf source files of the service.
///
/// * `#[service_factory(implements("path_1", "path_2"))]`
///
/// Path list to the interfaces which have been implemented by the service.
///
/// ## Optional
///
/// * `#[service_factory(crate = "path")]`
///
/// Prefix of the `exonum` crate has two main values - "crate" or "exonum". The default value is "exonum".
///
/// * `#[service_factory(artifact_name = "string")]`
///
/// Override artifact name, by default it uses crate name.
///
/// * `#[service_factory(artifact_version = "string")]`
///
/// Override artifact version, by default it uses crate version.
///
/// * `#[service_factory(with_constructor = "path")]`
///
/// Override service constructor by the custom function with the following signature:
///
/// `Fn(&ServiceFactoryImpl) -> Box<dyn Service>`.
///
/// * `#[service_factory(service_name = "string")]`
///
/// Use the specified service name for the ServiceDispatcher derivation instead of the struct name.
#[proc_macro_derive(ServiceFactory, attributes(service_factory))]
pub fn service_factory(input: TokenStream) -> TokenStream {
    service_factory::impl_service_factory(input)
}

/// Derives an Exonum service interface for the specified trait.
///
/// See the documentation of the Exonum crate for more information.
///
/// # Attributes:
///
/// ## Optional
///
/// * `#[exonum_interface(crate = "path")]`
///
/// Prefix of the `exonum` crate has two main values - "crate" or "exonum". The default value is "exonum".
#[proc_macro_attribute]
pub fn exonum_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    exonum_interface::impl_exonum_interface(attr, item)
}

/// Implements `From<MyError> for ExecutionError` conversion for the given enum.
///
/// Enumeration should have an explicit discriminant for each error kind.
/// Derives `Display` and `Fail` traits using documentation comments for each error kind.
///
/// # Attributes:
///
/// ## Optional
///
/// ```text
/// #[execution_error(crate = "path")]
/// ```
///
/// Prefix of the `exonum` crate has two main values - `crate` or `exonum`. The default value
/// is `exonum`.
#[proc_macro_derive(ServiceFail, attributes(service_fail))]
pub fn service_fail(input: TokenStream) -> TokenStream {
    service_fail::impl_service_fail(input)
}

pub(crate) fn find_meta_attrs(name: &str, args: &[Attribute]) -> Option<NestedMeta> {
    args.as_ref()
        .iter()
        .filter_map(|a| a.parse_meta().ok())
        .find(|m| m.path().is_ident(name))
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
