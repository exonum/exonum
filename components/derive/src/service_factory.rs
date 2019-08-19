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

use darling::FromDeriveInput;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use semver::Version;
use syn::{DeriveInput, Ident, Path};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(exonum), forward_attrs(allow, doc, cfg))]
#[darling(default)]
struct ServiceFactory {
    ident: Ident,
    #[darling(rename = "crate")]
    cr: Path,
    artifact_name: Option<String>,
    artifact_version: Option<String>,
    proto_sources: Option<Path>,
    with_constructor: Option<Path>,
}

impl Default for ServiceFactory {
    fn default() -> Self {
        Self {
            ident: Ident::new("unreachable", Span::call_site()),
            cr: syn::parse_str("exonum").unwrap(),
            artifact_name: None,
            artifact_version: None,
            proto_sources: None,
            with_constructor: None,
        }
    }
}

impl ServiceFactory {
    fn artifact_name(&self) -> impl ToTokens {
        if let Some(ref artifact_name) = self.artifact_name {
            // TODO check artifact name
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

    fn artifact_id(&self) -> impl ToTokens {
        let artifact_name = self.artifact_name();
        let artifact_version = self.artifact_version();
        quote! { concat!(#artifact_name, ":", #artifact_version) }
    }

    fn service_constructor(&self) -> impl ToTokens {
        if let Some(ref path) = self.with_constructor {
            quote! { #path(self) }
        } else {
            quote! { Box::new(Self) }
        }
    }

    fn artifact_protobuf_spec(&self) -> impl ToTokens {
        let cr = &self.cr;
        let proto_sources_mod = self
            .proto_sources
            .as_ref()
            .expect("`proto_sources` attribute is not set properly");

        quote! {
            #cr::runtime::ArtifactProtobufSpec {
                sources: #proto_sources_mod::PROTO_SOURCES.as_ref(),
            }
        }
    }
}

impl ToTokens for ServiceFactory {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.ident;
        let cr = &self.cr;
        let artifact_id = self.artifact_id();
        let artifact_protobuf_spec = self.artifact_protobuf_spec();
        let service_constructor = self.service_constructor();

        let expanded = quote! {
            impl #cr::runtime::rust::ServiceFactory for #name {
                fn artifact_id(&self) -> #cr::runtime::rust::RustArtifactId {
                    #artifact_id.parse().unwrap()
                }

                fn artifact_protobuf_spec(&self) -> #cr::runtime::ArtifactProtobufSpec {
                    #artifact_protobuf_spec
                }

                fn create_instance(&self) -> Box<dyn #cr::runtime::rust::Service> {
                    #service_constructor
                }
            }
        };
        tokens.extend(expanded)
    }
}

pub fn implement_service_factory(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let service_factory = ServiceFactory::from_derive_input(&input)
        .unwrap_or_else(|e| panic!("ServiceFactory: {}", e));
    let tokens = quote! {#service_factory};
    tokens.into()
}
