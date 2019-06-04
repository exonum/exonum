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

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, AttributeArgs, FnArg, Ident, ItemTrait, NestedMeta, TraitItem,
    TraitItemMethod, Type,
};

struct ServiceMethodDescriptor {
    name: Ident,
    arg_type: Type,
    id: u32,
}

fn impl_dispatch_method(methods: &[ServiceMethodDescriptor], cr: &dyn ToTokens) -> impl ToTokens {
    let match_arms = methods
        .iter()
        .map(|ServiceMethodDescriptor { name, arg_type, id }| {
            quote! {
                #id => {
                    let arg: #arg_type = exonum_merkledb::BinaryValue::from_bytes(payload.into())?;
                    Ok(self.#name(ctx,arg))
                }
            }
        });

    quote! {
        #[doc(hidden)]
        fn _dispatch(
                &self,
                ctx: #cr::runtime::rust::TransactionContext,
                method: #cr::messages::MethodId,
                payload: &[u8]
            ) -> Result<Result<(), #cr::runtime::error::ExecutionError>, failure::Error> {
            match method {
                #( #match_arms )*
                _ => failure::bail!("Method not found"),
            }
        }
    }
}

fn implement_into_service_tx(
    methods: &[ServiceMethodDescriptor],
    cr: &dyn ToTokens,
) -> impl quote::ToTokens {
    let into_service_tx = methods
        .iter()
        .map(|ServiceMethodDescriptor { arg_type, id, .. }| {
            quote! {
                impl From<#arg_type> for #cr::messages::ServiceTransaction {
                    fn from(value: #arg_type) -> Self {
                        let arg = exonum_merkledb::BinaryValue::into_bytes(value);
                        #cr::messages::ServiceTransaction::from_raw_unchecked(#id as u16, arg)
                    }
                }
            }
        });

    quote! {
        #( #into_service_tx )*
    }
}

pub fn impl_service_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut trait_item = parse_macro_input!(item as ItemTrait);
    let args = parse_macro_input!(attr as AttributeArgs);
    let meta_attrs = args
        .into_iter()
        .filter_map(|a| match a {
            NestedMeta::Meta(m) => Some(m),
            _ => None,
        })
        .collect::<Vec<_>>();
    let cr = super::get_exonum_types_prefix(&meta_attrs);

    let methods = trait_item
        .items
        .iter()
        .filter(|item| match item {
            TraitItem::Method(_) => true,
            _ => false,
        })
        .enumerate()
        .map(|(n, item)| {
            let method = match item {
                TraitItem::Method(m) => m,
                _ => unreachable!(),
            };
            let mut method_args_iter = method.sig.decl.inputs.iter();

            method_args_iter
                .next()
                .and_then(|arg| match arg {
                    FnArg::SelfRef(_) => Some(()),
                    _ => None,
                })
                .expect("Expected &self or &mut self as an argument");

            method_args_iter
                .next()
                .expect("Expected TransactionContext argument");

            let arg_type = method_args_iter
                .next()
                .map(|arg| match arg {
                    FnArg::Captured(captured) => captured.ty.clone(),
                    _ => panic!("Expected captured arg"),
                })
                .expect("Expected argument with transaction");

            if method_args_iter.next().is_some() {
                panic!("Function should have only one argument for transaction");
            }

            ServiceMethodDescriptor {
                name: method.sig.ident.clone(),
                id: n as u32,
                arg_type,
            }
        })
        .collect::<Vec<_>>();

    let dispatch_method: TraitItemMethod = {
        let method_code = impl_dispatch_method(&methods, &cr).into_token_stream();
        syn::parse(method_code.into()).expect("Can't parse trait item method")
    };
    trait_item.items.push(TraitItem::Method(dispatch_method));

    let into_service_tx = implement_into_service_tx(&methods, &cr);

    let expanded = quote! {
        #trait_item
        #into_service_tx
    };
    expanded.into()
}
