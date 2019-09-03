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

use darling::FromMeta;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, Attribute, AttributeArgs, FnArg, Ident, ItemTrait, NestedMeta, TraitItem,
    TraitItemMethod, Type,
};

use std::convert::TryFrom;

use super::{find_exonum_meta, CratePath};

#[derive(Debug)]
struct ServiceMethodDescriptor {
    name: Ident,
    arg_type: Box<Type>,
    id: u32,
}

impl TryFrom<(usize, &TraitItem)> for ServiceMethodDescriptor {
    type Error = darling::Error;

    fn try_from(value: (usize, &TraitItem)) -> Result<Self, Self::Error> {
        let method = match value.1 {
            TraitItem::Method(m) => m,
            _ => unreachable!(),
        };
        let mut method_args_iter = method.sig.inputs.iter();

        method_args_iter
            .next()
            .and_then(|arg| match arg {
                FnArg::Receiver(_) => Some(()),
                _ => None,
            })
            .ok_or_else(|| {
                darling::Error::unexpected_type("Expected `&self` or `&mut self` as an argument")
            })?;

        method_args_iter.next().ok_or_else(|| {
            darling::Error::unexpected_type("Expected `TransactionContext` argument")
        })?;

        let arg_type = method_args_iter
            .next()
            .ok_or_else(|| darling::Error::unexpected_type("Expected argument with transaction"))
            .and_then(|arg| match arg {
                FnArg::Typed(arg) => Ok(arg.ty.clone()),
                _ => Err(darling::Error::unexpected_type("Expected captured arg")),
            })?;

        if method_args_iter.next().is_some() {
            return Err(darling::Error::unsupported_format(
                "Function should have only one argument for transaction",
            ));
        }

        Ok(ServiceMethodDescriptor {
            name: method.sig.ident.clone(),
            id: value.0 as u32,
            arg_type,
        })
    }
}

#[derive(Debug, FromMeta)]
#[darling(default)]
struct ExonumServiceAttrs {
    #[darling(rename = "crate")]
    cr: CratePath,
    interface: Option<String>,
}

impl Default for ExonumServiceAttrs {
    fn default() -> Self {
        Self {
            cr: CratePath::default(),
            interface: None,
        }
    }
}

impl TryFrom<&[Attribute]> for ExonumServiceAttrs {
    type Error = darling::Error;

    fn try_from(args: &[Attribute]) -> Result<Self, Self::Error> {
        find_exonum_meta(args)
            .map(|meta| Self::from_nested_meta(&meta))
            .unwrap_or_else(|| Ok(Self::default()))
    }
}

#[derive(Debug)]
struct ExonumService {
    item_trait: ItemTrait,
    attrs: ExonumServiceAttrs,
    methods: Vec<ServiceMethodDescriptor>,
}

impl ExonumService {
    fn new(item_trait: ItemTrait, args: Vec<NestedMeta>) -> Result<Self, darling::Error> {
        let methods = item_trait
            .items
            .iter()
            .enumerate()
            .filter_map(|x| match x.1 {
                TraitItem::Method(_) => Some(ServiceMethodDescriptor::try_from(x)),
                _ => None,
            })
            .try_fold(Vec::new(), |mut v, x| {
                v.push(x?);
                Ok(v)
            })?;

        Ok(Self {
            item_trait,
            attrs: ExonumServiceAttrs::from_list(&args)?,
            methods,
        })
    }

    fn interface_name(&self) -> &str {
        self.attrs
            .interface
            .as_ref()
            .map(String::as_ref)
            .unwrap_or_default()
    }

    fn impl_dispatch_method(&self) -> impl ToTokens {
        let trait_name = &self.item_trait.ident;
        let cr = &self.attrs.cr;

        let match_arms = self.methods.iter().map(
            |ServiceMethodDescriptor { name, arg_type, id }| {
                quote! {
                    #id => {
                        let bytes = payload.into();
                        let arg: #arg_type = exonum_merkledb::BinaryValue::from_bytes(bytes)
                            .map_err(|error_msg|
                                (
                                    #cr::runtime::rust::error::Error::ArgumentsParseError,
                                    format!("Unable to parse argument for the `{}#{}` method. {}",
                                        stringify!(#trait_name), stringify!(#name), error_msg
                                    )
                                )
                            )?;
                        self.#name(ctx,arg).map_err(From::from)
                    }
                }
            },
        );

        quote! {
            #[doc(hidden)]
            fn dispatch(
                    &self,
                    ctx: #cr::runtime::rust::TransactionContext,
                    method: #cr::runtime::MethodId,
                    payload: &[u8]
                ) -> Result<(), #cr::runtime::error::ExecutionError> {
                match method {
                    #( #match_arms )*
                    other => {
                        let kind = #cr::runtime::dispatcher::Error::NoSuchMethod;
                        let message = format!(
                            "Method with ID {} is absent in the '{}' interface of the instance `{}`",
                            other, stringify!(#trait_name), ctx.instance.name,
                        );
                        Err((kind, message)).map_err(From::from)
                    }
                }
            }
        }
    }

    fn impl_transactions(&self) -> impl ToTokens {
        let cr = &self.attrs.cr;
        let trait_name = &self.item_trait.ident;
        let interface_name = self.interface_name();

        let transactions_for_methods =
            self.methods
                .iter()
                .map(|ServiceMethodDescriptor { arg_type, id, .. }| {
                    quote! {
                        impl #cr::runtime::rust::Transaction for #arg_type {
                            type Service = &'static dyn #trait_name;

                            const INTERFACE_NAME: &'static str = #interface_name;
                            const METHOD_ID: #cr::runtime::MethodId = #id;
                        }
                    }
                });

        quote! {
            #( #transactions_for_methods )*
        }
    }

    fn impl_interface_describe(&self) -> impl ToTokens {
        let cr = &self.attrs.cr;
        let trait_name = &self.item_trait.ident;
        let interface_name = self.interface_name();

        quote! {
            impl #cr::runtime::rust::service::InterfaceDescribe for dyn #trait_name {
                const INTERFACE_NAME: &'static str = #interface_name;
            }
        }
    }

    fn item_trait(&self) -> impl ToTokens {
        let mut item_trait = self.item_trait.clone();
        let dispatch_method: TraitItemMethod = {
            let method_code = self.impl_dispatch_method().into_token_stream();
            syn::parse(method_code.into()).expect("Can't parse trait item method")
        };
        item_trait.items.push(TraitItem::Method(dispatch_method));
        item_trait
    }
}

impl ToTokens for ExonumService {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let item_trait = self.item_trait();
        let impl_transactions = self.impl_transactions();
        let impl_interface_describe = self.impl_interface_describe();

        let expanded = quote! {
            #item_trait
            #impl_transactions
            #impl_interface_describe
        };
        tokens.extend(expanded);
    }
}

pub fn impl_service_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_trait = parse_macro_input!(item as ItemTrait);
    let attrs = parse_macro_input!(attr as AttributeArgs);

    let exonum_service =
        ExonumService::new(item_trait, attrs).unwrap_or_else(|e| panic!("ExonumService: {}", e));

    let tokens = quote! {#exonum_service};
    tokens.into()
}
