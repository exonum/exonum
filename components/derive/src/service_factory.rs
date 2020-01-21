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

use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use semver::Version;
use syn::{DeriveInput, Generics, Ident, Path};

use super::RustRuntimeCratePath;

fn is_allowed_artifact_name_char(c: u8) -> bool {
    match c {
        b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'.' | b'_' | b'/' => true,
        _ => false,
    }
}

/// Check that the artifact name contains only allowed characters and is not empty.
///
/// Only these combination of symbols are allowed:
///
/// `[0..9]`, `[a-z]`, `[A-Z]`, `/`, `_`, `-`, `.`.
fn check_artifact_name(name: impl AsRef<[u8]>) -> bool {
    name.as_ref()
        .iter()
        .copied()
        .all(is_allowed_artifact_name_char)
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(service_factory), forward_attrs(allow, doc, cfg))]
struct ServiceFactory {
    ident: Ident,
    #[darling(rename = "crate", default)]
    cr: RustRuntimeCratePath,
    #[darling(default)]
    artifact_name: Option<String>,
    #[darling(default)]
    artifact_version: Option<String>,
    #[darling(default)]
    proto_sources: Option<Path>,
    #[darling(default)]
    service_constructor: Option<Path>,
    #[darling(default)]
    generics: Generics,
}

impl ServiceFactory {
    fn artifact_name(&self) -> impl ToTokens {
        if let Some(ref artifact_name) = self.artifact_name {
            // Check that artifact name contains only allowed characters and is not empty.
            // It's better to check it now, than wait for panic in the runtime.
            if !check_artifact_name(artifact_name) {
                panic!(
                    "Wrong characters used in artifact name. Use only: a-zA-Z0-9 and one of /_.-"
                )
            }

            quote! { #artifact_name }
        } else {
            quote! { env!("CARGO_PKG_NAME") }
        }
    }

    fn artifact_version(&self) -> impl ToTokens {
        if let Some(ref artifact_version) = self.artifact_version {
            // Check that artifact version is semver compatible.
            Version::parse(artifact_version).expect("Unable to parse artifact version");
            quote! { #artifact_version }
        } else {
            quote! { env!("CARGO_PKG_VERSION") }
        }
    }

    fn service_constructor(&self) -> impl ToTokens {
        if let Some(ref path) = self.service_constructor {
            quote! { #path(self) }
        } else {
            quote! { Box::new(Self) }
        }
    }

    fn artifact_protobuf_spec(&self) -> impl ToTokens {
        let cr = &self.cr;
        if let Some(ref proto_sources_mod) = self.proto_sources {
            quote! {
                #cr::ArtifactProtobufSpec::new(
                    #proto_sources_mod::PROTO_SOURCES.as_ref(),
                    #proto_sources_mod::INCLUDES.as_ref()
                )
            }
        } else {
            quote! {
                #cr::ArtifactProtobufSpec {
                    sources: vec![],
                    includes: vec![],
                }
            }
        }
    }
}

impl ToTokens for ServiceFactory {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.ident;
        let cr = &self.cr;
        let artifact_name = self.artifact_name();
        let artifact_version = self.artifact_version();
        let artifact_protobuf_spec = self.artifact_protobuf_spec();
        let service_constructor = self.service_constructor();
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();

        let expanded = quote! {
            impl #impl_generics #cr::ServiceFactory for #name #ty_generics #where_clause {
                fn artifact_id(&self) -> #cr::_reexports::ArtifactId {
                    #cr::_reexports::ArtifactId::new(
                        #cr::_reexports::RuntimeIdentifier::Rust as u32,
                        #artifact_name.to_string(),
                        #artifact_version.parse().expect("Cannot parse artifact version"),
                    ).expect("Invalid artifact identifier")
                }

                fn artifact_protobuf_spec(&self) -> #cr::ArtifactProtobufSpec {
                    #artifact_protobuf_spec
                }

                fn create_instance(&self) -> Box<dyn #cr::Service> {
                    #service_constructor
                }
            }
        };
        tokens.extend(expanded)
    }
}

pub fn impl_service_factory(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let service_factory = ServiceFactory::from_derive_input(&input)
        .unwrap_or_else(|e| panic!("ServiceFactory: {}", e));
    let tokens = quote! {#service_factory};
    tokens.into()
}
