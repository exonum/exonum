// Copyright 2020 The Exonum Team
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
mod execution_fail;
mod exonum_interface;
mod require_artifact;
mod service_dispatcher;
mod service_factory;

use darling::FromMeta;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{Attribute, NestedMeta};

/// Derives `BinaryValue` trait. The target type must implement (de)serialization logic,
/// which should be provided externally.
///
/// The trait currently supports two codecs:
///
/// - Protobuf serialization (used by default) via `exonum-proto` crate and its `ProtobufConvert`
///   trait.
/// - `bincode` serialization via the eponymous crate. Switched on by the
///   `#[binary_value(codec = "bincode")]` attribute. Beware that `bincode` format is not as
///   forward / backward compatible as Protobuf; hence, this codec is better suited for tests
///   than for production code.
///
/// # Container Attributes
///
/// ## `codec`
///
/// Selects the serialization codec to use. Allowed values are `protobuf` (used by default)
/// and `bincode`.
///
/// # Examples
///
/// With Protobuf serialization:
///
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
///
/// With `bincode` serialization:
///
/// ```ignore
/// #[derive(Clone, Debug, Serialize, Deserialize, BinaryValue)]
/// #[binary_value(codec = "bincode")]
/// pub struct Wallet {
///     pub username: PublicKey,
///     /// Current balance of the wallet.
///     pub balance: u64,
/// }
///
/// let wallet = Wallet {
///     username: "Alice".to_owned(),
///     balance: 100,
/// };
/// let bytes = wallet.to_bytes();
/// ```
#[proc_macro_derive(BinaryValue, attributes(binary_value))]
pub fn binary_value(input: TokenStream) -> TokenStream {
    db_traits::impl_binary_value(input)
}

/// Derives `ObjectHash` trait. The target type must implement `BinaryValue` trait.
///
/// # Example
///
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
/// let wallet = Wallet {
///     pub_key: KeyPair::random().public_key(),
///     balance: 100,
/// };
/// let hash = wallet.object_hash();
/// ```
#[proc_macro_derive(ObjectHash)]
pub fn object_hash(input: TokenStream) -> TokenStream {
    db_traits::impl_object_hash(input)
}

/// Derives `FromAccess` trait.
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

/// Derives `ServiceDispatcher` trait.
///
/// # Container Attributes
///
/// ## `implements`
///
/// ```text
/// #[service_dispatcher(implements("path_1", "path_2"))]
/// ```
///
/// List of the interfaces which have been implemented by the service. If omitted, it's implied
/// that the service does not implement interfaces.
///
/// ## `crate`
///
/// ```text
/// #[service_dispatcher(crate = "path")]
/// ```
///
/// Prefix of the `exonum` crate has two main values - `crate` or `exonum`. The default value
/// is `exonum`.
#[proc_macro_derive(ServiceDispatcher, attributes(service_dispatcher))]
pub fn service_dispatcher(input: TokenStream) -> TokenStream {
    service_dispatcher::impl_service_dispatcher(input)
}

/// Derives `ServiceFactory` trait.
///
/// # Container Attributes
///
/// ## `proto_sources`
///
/// ```text
/// #[service_factory(proto_sources = "path")]
/// ```
///
/// Path to the module that was generated by the build script, which
/// contains the original Protobuf source files of the service. If omitted, no Protobuf sources
/// will be included with the service artifact.
///
/// ## `crate`
///
/// ```text
/// #[service_factory(crate = "path")]
/// ```
///
/// Prefix of the `exonum` crate has two main values - `crate` or `exonum`. The default value
/// is `exonum`.
///
/// ## `artifact_name`
///
/// ```text
/// #[service_factory(artifact_name = "string")]
/// ```
///
/// Overrides the artifact name, which is set to the crate name by default.
///
/// ## `artifact_version`
///
/// ```text
/// #[service_factory(artifact_version = "string")]
/// ```
///
/// Overrides the artifact version, which is set to the crate version by default.
///
/// ## `with_constructor`
///
/// ```text
/// #[service_factory(with_constructor = "path")]
/// ```
///
/// Overrides service constructor by a custom function with the following signature:
///
/// ```text
/// fn(&ServiceFactoryImpl) -> Box<dyn Service>
/// ```
#[proc_macro_derive(ServiceFactory, attributes(service_factory))]
pub fn service_factory(input: TokenStream) -> TokenStream {
    service_factory::impl_service_factory(input)
}

/// Derives an Exonum service interface for the specified trait.
///
/// See the documentation of the Exonum crate for more information.
///
/// # Attributes
///
/// ## `crate`
///
/// ```text
/// #[exonum_interface(crate = "path")]
/// ```
///
/// Prefix of the `exonum` crate has two main values - `crate` or `exonum`. The default value
/// is `exonum`.
///
/// ## `removed_method_ids`
///
/// ```text
/// #[exonum_interface(removed_method_ids(0, 2, 5))]
/// ```
///
/// Marks methods with the following IDs as removed. An attempt to invoke
/// the method with corresponding ID will always result in an error.
///
/// Using this attribute is a recommended way to remove methods from interface, since it
/// guarantees that method ID won't be reused.
///
/// This attribute cannot be used with `auto_ids` attribute set.
///
/// ## `id_auto_increment`
///
/// ```text
/// #[exonum_interface(auto_ids)]
/// ```
///
/// Enables automatic ID assignment for interface methods. This may be useful for writing tests,
/// but not recommended for production code.
///
/// # Method attributes
///
/// ## `interface_method`
///
/// ```test
/// #[interface_method(id = 0)]
/// ```
///
/// All the method in the trait with `exonum_interface` attribute should have `interface_method`
/// attribute with unsigned integer value. All the method IDs should be unique.
#[proc_macro_attribute]
pub fn exonum_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    exonum_interface::impl_exonum_interface(attr, item)
}

/// Meta-information attribute for interface methods.
///
/// # Fields
///
/// ## `id` (required)
///
/// ```text
/// #[interface_method(id = 0)]
/// ```
///
/// Numeric identifier of the method. Should be unique for every method in the trait.
///
/// Using this attribute is a recommended way to remove methods from interface, since it
/// guarantees that method ID won't be reused.
#[proc_macro_attribute]
pub fn interface_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // We don't modify the input stream, since `interface_method` attribute only
    // provides additional metadata for `exonum_interface` attribute.
    //
    // This however should be a `proc_macro_attribute`, so rust compiler won't complain about
    // unknown attribute.
    item
}

/// Implements `ExecutionFail` trait for the given enum. Additionally,
/// `From<MyEnum> for ExecutionError` conversion is implemented, allowing to use errors
/// in the service code.
///
/// Enumeration should have an explicit discriminant for each error kind.
/// The documentation comments for each error kind are used to derive the `Display` trait.
///
/// # Container Attributes
///
/// ## `crate`
///
/// ```text
/// #[execution_fail(crate = "path")]
/// ```
///
/// Prefix of the `exonum` crate has two main values - `crate` or `exonum`. The default value
/// is `exonum`.
///
/// ## `kind`
///
/// ```text
/// #[execution_fail(kind = "runtime")]
/// ```
///
/// Error kind with the following possible values: `service`, `runtime`. The default value is
/// `service`.
#[proc_macro_derive(ExecutionFail, attributes(execution_fail))]
pub fn execution_fail(input: TokenStream) -> TokenStream {
    execution_fail::impl_execution_fail(input)
}

/// Implements `RequireArtifact` trait for the given struct or enum. The target type may
/// be generic over type parameters.
///
/// # Container Attributes
///
/// ## `crate`
///
/// ```text
/// #[require_artifact(crate = "path")]
/// ```
///
/// Prefix of the `exonum` crate has two main values - `crate` or `exonum`. The default value
/// is `exonum`.
///
/// ## `name`
///
/// ```text
/// #[require_artifact(name = "artifact_name")]
/// ```
///
/// Name of the artifact. If omitted, will be set to the name of the crate.
///
/// ## `version`
///
/// ```text
/// #[require_artifact(version = "^1.3")]
/// ```
///
/// [Semantic version requirement] on the artifact. If omitted, will be set to be semver-compatible
/// with the current version of the crate. Depending on the use case, this may be too limiting;
/// e.g., if a certain interface was defined in v1.0.0, `version = "^1"` may be explicitly specified
/// in all the following crate releases.
///
/// [Semantic version requirement]: https://docs.rs/semver/0.9.0/semver/#requirements
#[proc_macro_derive(RequireArtifact, attributes(require_artifact))]
pub fn require_artifact(input: TokenStream) -> TokenStream {
    require_artifact::impl_require_artifact(input)
}

pub(crate) fn find_meta_attrs(name: &str, args: &[Attribute]) -> Option<NestedMeta> {
    args.as_ref()
        .iter()
        .filter_map(|a| a.parse_meta().ok())
        .find(|m| m.path().is_ident(name))
        .map(NestedMeta::from)
}

#[derive(Debug, FromMeta)]
#[darling(default)]
struct MainCratePath(syn::Path);

impl Default for MainCratePath {
    fn default() -> Self {
        Self(syn::parse_str("exonum").unwrap())
    }
}

impl ToTokens for MainCratePath {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

#[derive(Debug, FromMeta)]
#[darling(default)]
struct RustRuntimeCratePath(syn::Path);

impl Default for RustRuntimeCratePath {
    fn default() -> Self {
        Self(syn::parse_str("exonum_rust_runtime").unwrap())
    }
}

impl ToTokens for RustRuntimeCratePath {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}
