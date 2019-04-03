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
use syn::{FnArg, Ident, ItemTrait, TraitItem, TraitItemMethod, Type};

struct ServiceMethodDescriptor {
    name: Ident,
    arg_type: Type,
    id: u32,
}

// TODO: currently it works only inside exonum crate, also refactor is needed.

pub fn impl_service_interface(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut trait_item: ItemTrait =
        syn::parse(item.clone()).expect("service_dispatcher can be used on traits only");

    let vec_items = trait_item
        .items
        .iter()
        .filter(|item| {
            if let TraitItem::Method(_) = item {
                true
            } else {
                false
            }
        })
        .enumerate()
        .map(|(n, item)| {
            let method = match item {
                TraitItem::Method(m) => m,
                _ => panic!("Wrong item type."),
            };
            let name = method.sig.ident.clone();
            let mut method_iter = method.sig.decl.inputs.iter();
            method_iter
                .next()
                .and_then(|arg| match arg {
                    FnArg::SelfRef(_) => Some(()),
                    _ => None,
                })
                .expect("Expected &self or &mut self as an argument");

            method_iter
                .next()
                .expect("Expected TransactionContext argument");

            let typ = method_iter
                .next()
                .map(|arg| match arg {
                    FnArg::Captured(captured) => captured.ty.clone(),
                    _ => panic!("Expected captured arg"),
                })
                .expect("Expected argument with transaction");

            if method_iter.next().is_some() {
                panic!("Function should have only one argument");
            }

            ServiceMethodDescriptor {
                name,
                id: n as u32,
                arg_type: typ,
            }
        })
        .collect::<Vec<_>>();

    let match_arms = vec_items
        .iter()
        .map(|ServiceMethodDescriptor { name, arg_type, id }| {
            quote! {
                #id => {
                    let arg: #arg_type = crate::messages::BinaryForm::decode(payload)?;
                    Ok(self.#name(ctx,arg))
                }
            }
        });

    let method_code = quote! {
            fn _dispatch(
                    &self,
                    ctx: crate::runtime::rust::TransactionContext,
                    method: crate::messages::MethodId,
                    payload: &[u8]
                ) -> Result<Result<(), crate::runtime::error::ExecutionError>, failure::Error> {
                match method {
                    #( #match_arms )*
                    _ => bail!("Method not found"),
                }
            }
    };
    let additional_method: TraitItemMethod =
        syn::parse(method_code.into()).expect("Can't parse trait item method");
    trait_item.items.push(TraitItem::Method(additional_method));

    trait_item.into_token_stream().into()
}
