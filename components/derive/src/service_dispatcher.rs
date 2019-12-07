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
use syn::{DeriveInput, Generics, Ident, Lit, NestedMeta, Path};

use super::CratePath;

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
#[darling(attributes(service_dispatcher), forward_attrs(allow, doc, cfg))]
struct ServiceDispatcher {
    ident: Ident,
    #[darling(rename = "crate", default)]
    cr: CratePath,
    #[darling(default)]
    implements: ServiceInterfaces,
    #[darling(default)]
    generics: Generics,
}

impl ToTokens for ServiceDispatcher {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let service_name = &self.ident;
        let cr = &self.cr;

        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();

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

        let expanded = quote! {
            impl #impl_generics #cr::runtime::rust::ServiceDispatcher for #service_name #ty_generics #where_clause  {
                fn call(
                    &self,
                    interface_name: &str,
                    method: #cr::runtime::MethodId,
                    ctx: #cr::runtime::rust::CallContext<'_>,
                    payload: &[u8],
                ) -> Result<(), #cr::runtime::error::ExecutionError> {
                    match interface_name {
                        #( #match_arms )*
                        other => Err(#cr::runtime::DispatcherError::NoSuchInterface.into()),
                    }
                }
            }
        };
        tokens.extend(expanded);
    }
}

pub fn impl_service_dispatcher(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let service_dispatcher = ServiceDispatcher::from_derive_input(&input)
        .unwrap_or_else(|e| panic!("ServiceDispatcher: {}", e));
    let tokens = quote! { #service_dispatcher };
    tokens.into()
}
