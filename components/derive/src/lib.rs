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
mod execution_error;
mod exonum_interface;
mod service_dispatcher;
mod service_factory;

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
    db_traits::binary_value(input)
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
    db_traits::object_hash(input)
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
/// Prefix of the `exonum` crate(usually "crate" or "exonum").
#[proc_macro_derive(ServiceDispatcher, attributes(service_dispatcher))]
pub fn service_dispatcher(input: TokenStream) -> TokenStream {
    service_dispatcher::implement_service_dispatcher(input)
}

/// Derive `ServiceFactory` and `ServiceDispatcher` traits.
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
/// * `#[service_factory(implements(""path_1", "path_2""))]`
///
/// Path list to the interfaces which have been implemented by the service.
///
/// ## Optional
///
/// * `#[service_factory(crate = "path")]`
///
/// Prefix of the `exonum` crate(usually "crate" or "exonum").
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
///
///
/// # Examples TODO Move to ServiceFactory documentation [ECR-3275]
///
/// Typical usage.
/// ```ignore
/// #[derive(ServiceFactory, ServiceDispatcher)]
/// #[service_dispatcher(implements("MyServiceInterface"))]
/// #[service_factory(proto_sources = "crate::proto")]
/// pub struct MyService;
/// ```
///
/// But if you have complex logic in service factory you can use custom constructor to create a
/// new service instances.
/// ```ignore
/// // Imagine that you have a stateful service like this
/// #[derive(Debug)]
/// pub struct TimeService {
///     /// Current time.
///     time: Arc<dyn TimeProvider>,
/// }
///
/// // You can implement service factory, but you cannot just derive `ServiceFactory`
/// // like in example bellow.
/// // To resolve this problem you can specify your own constructor for the service instance.
/// #[derive(Debug, ServiceDispatcher, ServiceFactory)]
/// #[service_dispatcher(implements("TimeServiceInterface"))]
/// #[service_factory(
///     proto_sources = "proto",
///     service_constructor = "TimeServiceFactory::create_instance",
///     service_name = "TimeService",
/// )]
/// pub struct TimeServiceFactory {
///     time_provider: Arc<dyn TimeProvider>,
/// }
///
/// // Arbitrary constructor implementation.
/// impl TimeServiceFactory {
///     fn create_instance(&self) -> Box<dyn Service> {
///         Box::new(TimeService {
///             time: self.time_provider.clone(),
///         })
///     }
/// }
/// ```
#[proc_macro_derive(ServiceFactory, attributes(service_factory))]
pub fn service_factory(input: TokenStream) -> TokenStream {
    service_factory::impl_service_factory(input)
}

/// Mark trait as an Exonum service interface.
#[proc_macro_attribute]
pub fn exonum_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    exonum_interface::impl_service_interface(attr, item)
}

/// Derive `Into<ExecutionError>` conversion for the specified enumeration.
///
/// Enumeration should have an explicit discriminant for each variant.
/// Also this macro derives `Display` trait using documentation comments of each variant.
///
/// # Examples
///
/// ```ignore
/// /// Error codes emitted by wallet transactions during execution.
/// #[derive(Debug, IntoExecutionError)]
/// pub enum Error {
///     /// Content hash already exists.
///     HashAlreadyExists = 0,
///     /// Unable to parse service configuration.
///     ConfigParseError = 1,
///     /// Time service with the specified name doesn't exist.
///     TimeServiceNotFound = 2,
/// }
/// ```
///
#[proc_macro_derive(IntoExecutionError, attributes(exonum))]
pub fn generate_into_execution_error(input: TokenStream) -> TokenStream {
    execution_error::implement_execution_error(input)
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
