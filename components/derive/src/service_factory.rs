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

use darling::{FromDeriveInput, FromMeta};
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use semver::Version;
use syn::{DeriveInput, Ident, Lit, NestedMeta, Path};

use super::CratePath;

fn is_allowed_latin1_char(c: u8) -> bool {
    match c {
          48..=57   // 0..9
        | 65..=90   // A..Z
        | 97..=122  // a..z
        | 45..=46   // -.
        | 95        // _
        | 58        // :
          => true,
        _ => false,
    }
}

/// Check that the artifact name contains only allowed characters and is not empty.
///
/// Only these combination of symbols are allowed:
///
/// `[0..9]`, `[a-z]`, `[A-Z]`, `_`, `-`, `.`, ':'
fn check_artifact_name(name: impl AsRef<[u8]>) -> bool {
    name.as_ref().iter().copied().all(is_allowed_latin1_char)
}

#[derive(Debug, Default)]
struct ServiceInterfaces(Vec<Path>);

impl FromMeta for ServiceInterfaces {
    fn from_string(value: &str) -> darling::Result<Self> {
        Path::from_string(value).map(|path| Self(vec![path]))
    }

    fn from_value(value: &Lit) -> darling::Result<Self> {
        Path::from_value(value).map(|path| Self(vec![path]))
    }

    fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
        items
            .iter()
            .map(|meta| match meta {
                NestedMeta::Lit(lit) => Path::from_value(lit),
                _ => Err(darling::Error::unsupported_format(
                    "Services should be in format: `implements(\"First\", \"Second\")`",
                )),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(exonum), forward_attrs(allow, doc, cfg))]
struct ServiceFactory {
    ident: Ident,
    #[darling(rename = "crate", default)]
    cr: CratePath,
    #[darling(default)]
    artifact_name: Option<String>,
    #[darling(default)]
    artifact_version: Option<String>,
    #[darling(default)]
    proto_sources: Option<Path>,
    #[darling(default)]
    service_constructor: Option<Path>,
    implements: ServiceInterfaces,
    #[darling(default)]
    service_name: Option<Ident>,
}

impl ServiceFactory {
    fn artifact_name(&self) -> impl ToTokens {
        if let Some(ref artifact_name) = self.artifact_name {
            // Check that artifact name contains only allowed characters and is not empty.
            if !check_artifact_name(artifact_name) {
                panic!("Wrong characters using in artifact name. Use: a-zA-Z0-9 and one of _-.:")
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

    fn artifact_id(&self) -> impl ToTokens {
        let artifact_name = self.artifact_name();
        let artifact_version = self.artifact_version();
        quote! { concat!(#artifact_name, ":", #artifact_version) }
    }

    fn service_constructor(&self) -> impl ToTokens {
        if let Some(ref path) = self.service_constructor {
            quote! { #path(self) }
        } else {
            quote! { Box::new(Self) }
        }
    }

    fn service_name(&self) -> impl ToTokens {
        self.service_name
            .clone()
            .unwrap_or_else(|| self.ident.clone())
    }

    fn artifact_protobuf_spec(&self) -> impl ToTokens {
        let cr = &self.cr;
        let proto_sources_mod = self
            .proto_sources
            .as_ref()
            .expect("`proto_sources` attribute is not set properly");

        quote! {
            #cr::runtime::ArtifactProtobufSpec::from(
                #proto_sources_mod::PROTO_SOURCES.as_ref(),
            )
        }
    }

    fn impl_service_dispatcher(&self) -> impl ToTokens {
        let cr = &self.cr;
        let dispatcher = self.service_name();

        let match_arms = self.implements.0.iter().map(|trait_name| {
            let interface_trait = quote! {
                <dyn #trait_name as #cr::runtime::rust::Interface>
            };

            quote! {
                #interface_trait::INTERFACE_NAME => {
                    #interface_trait::dispatch(self, ctx, method, payload)
                }
            }
        });

        quote! {
            impl #cr::runtime::rust::ServiceDispatcher for #dispatcher {
                fn call(
                    &self,
                    interface_name: &str,
                    method: #cr::runtime::MethodId,
                    ctx: #cr::runtime::rust::TransactionContext,
                    payload: &[u8],
                ) -> Result<(), #cr::runtime::error::ExecutionError> {
                    match interface_name {
                        #( #match_arms )*
                        other => {
                            let message = format!(
                                "Service instance `{}` does not implement a `{}` interface.",
                                ctx.instance.name,
                                other
                            );
                            Err(#cr::runtime::DispatcherError::no_such_interface(message))
                        }
                    }
                }
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
        let service_dispatcher = self.impl_service_dispatcher();

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

            #service_dispatcher
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
