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

use darling::{FromDeriveInput, FromMeta};
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{DeriveInput, Generics, Ident, Lit, Meta, NestedMeta, Path};

use super::RustRuntimeCratePath;

#[derive(Debug)]
struct ServiceInterface {
    path: Path,
    is_raw: bool,
}

impl FromMeta for ServiceInterface {
    fn from_meta(meta: &Meta) -> darling::Result<Self> {
        match meta {
            Meta::NameValue(name_and_value) => {
                let flag_name = name_and_value.path.get_ident().map(ToString::to_string);
                if flag_name.as_ref().map(String::as_str) == Some("raw") {
                    let mut this = Self::from_value(&name_and_value.lit)?;
                    this.is_raw = true;
                    Ok(this)
                } else {
                    let msg = "Unsupported flag (supported flags: `raw`)";
                    Err(darling::Error::custom(msg).with_span(&name_and_value.path))
                }
            }
            _ => {
                let msg = "Unsupported interface format; use `\"InterfaceName\" or \
                           `raw = \"InterfaceName\"`";
                Err(darling::Error::custom(msg).with_span(meta))
            }
        }
    }

    fn from_string(value: &str) -> darling::Result<Self> {
        Ok(Self {
            path: Path::from_string(value)?,
            is_raw: false,
        })
    }
}

#[derive(Debug, Default)]
struct ServiceInterfaces(Vec<ServiceInterface>);

impl FromMeta for ServiceInterfaces {
    fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
        items
            .iter()
            .map(ServiceInterface::from_nested_meta)
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    fn from_value(value: &Lit) -> darling::Result<Self> {
        ServiceInterface::from_value(value).map(|interface| Self(vec![interface]))
    }

    fn from_string(value: &str) -> darling::Result<Self> {
        ServiceInterface::from_string(value).map(|interface| Self(vec![interface]))
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(service_dispatcher), forward_attrs(allow, doc, cfg))]
struct ServiceDispatcher {
    ident: Ident,
    #[darling(rename = "crate", default)]
    cr: RustRuntimeCratePath,
    #[darling(default)]
    implements: ServiceInterfaces,
    #[darling(default)]
    generics: Generics,
}

/// Transforms the interface object into token stream representing the interface trait.
fn interface_trait(
    cr: &RustRuntimeCratePath,
    interface: &ServiceInterface,
) -> proc_macro2::TokenStream {
    let ctx = quote!(#cr::CallContext<'_>);
    let res = quote!(std::result::Result<(), #cr::ExecutionError>);
    let trait_name = &interface.path;
    let interface_trait = if interface.is_raw {
        quote!(dyn #trait_name)
    } else {
        quote!(dyn #trait_name<#ctx, Output = #res>)
    };
    quote!(<#interface_trait as #cr::Interface>)
}

impl ToTokens for ServiceDispatcher {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let service_name = &self.ident;
        let cr = &self.cr;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let ctx = quote!(#cr::CallContext<'_>);

        // Match arms for implementing `call` method.
        // Essentially here we provide a dispatching map for interface calls.
        let match_arms = self.implements.0.iter().map(|interface| {
            let interface_trait = interface_trait(&cr, interface);

            quote! {
                #interface_trait::INTERFACE_NAME => {
                    #interface_trait::dispatch(self, ctx, method, payload)
                }
            }
        });

        // List of interface names, available for service.
        let interface_names = self.implements.0.iter().map(|interface| {
            let interface_trait = interface_trait(&cr, interface);

            quote! { #interface_trait::INTERFACE_NAME }
        });

        // Implementation of `ServiceDispatcher` trait for service type.
        let expanded = quote! {
            impl #impl_generics #cr::ServiceDispatcher for #service_name #ty_generics #where_clause  {
                fn call(
                    &self,
                    interface_name: &str,
                    method: #cr::MethodId,
                    ctx: #ctx,
                    payload: &[u8],
                ) -> Result<(), #cr::ExecutionError> {
                    match interface_name {
                        #( #match_arms )*
                        _ => Err(#cr::CommonError::NoSuchInterface.into()),
                    }
                }

                fn interfaces(&self) -> Vec<String> {
                    let interface_names = [#( (#interface_names) ),*];
                    let iter: std::slice::Iter<&'static str> = interface_names.iter();
                    iter.map(ToString::to_string).collect::<Vec<String>>()
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
