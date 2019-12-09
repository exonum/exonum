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
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, Attribute, AttributeArgs, FnArg, Ident, ItemTrait, NestedMeta, Receiver,
    ReturnType, TraitItem, TraitItemMethod, Type,
};

use std::convert::TryFrom;

use super::{find_meta_attrs, CratePath};

#[derive(Debug)]
struct ServiceMethodDescriptor {
    name: Ident,
    arg_type: Box<Type>,
    id: u32,
}

const INVALID_METHOD_MSG: &str =
    "Interface method should have form `fn foo(&self, ctx: Ctx, arg: Bar) -> Self::Output`";

impl ServiceMethodDescriptor {
    fn try_from(index: usize, method: &mut TraitItemMethod) -> Result<Self, darling::Error> {
        let mut method_args_iter = method.sig.inputs.iter();

        if let Some(arg) = method_args_iter.next() {
            match arg {
                FnArg::Receiver(Receiver {
                    reference: Some(_),
                    mutability: None,
                    ..
                }) => {}
                _ => {
                    let msg = "The first argument in an interface method should be `&self`";
                    return Err(darling::Error::custom(msg).with_span(&arg));
                }
            }
        } else {
            return Err(darling::Error::custom(INVALID_METHOD_MSG).with_span(method));
        }

        method_args_iter
            .next()
            .ok_or_else(|| darling::Error::custom(INVALID_METHOD_MSG).with_span(method))?;

        let arg_type = method_args_iter
            .next()
            .ok_or_else(|| darling::Error::custom(INVALID_METHOD_MSG).with_span(method))
            .and_then(|arg| match arg {
                FnArg::Typed(arg) => Ok(arg.ty.clone()),
                _ => Err(darling::Error::custom(INVALID_METHOD_MSG).with_span(method)),
            })?;

        if method_args_iter.next().is_some() {
            return Err(darling::Error::custom(INVALID_METHOD_MSG).with_span(method));
        }

        if let ReturnType::Type(_, ref mut ty) = method.sig.output {
            let ty: &mut Type = ty.as_mut();
            match ty {
                Type::Infer(_) => {
                    *ty = syn::parse_quote!(Self::Output);
                }
                Type::Path(_) => {} // FIXME: use more thorough check
                _ => {
                    let msg = "Unsupported return type; use `_` or `Self::Output`";
                    return Err(darling::Error::custom(msg).with_span(ty));
                }
            }
        } else {
            return Err(darling::Error::custom(INVALID_METHOD_MSG).with_span(&method.sig.output));
        }

        Ok(ServiceMethodDescriptor {
            name: method.sig.ident.clone(),
            id: index as u32, // TODO: allow to parse `method_id` from attrs
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
        find_meta_attrs("exonum", args)
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
    fn new(mut item_trait: ItemTrait, args: Vec<NestedMeta>) -> Result<Self, darling::Error> {
        // FIXME: extract context generic from the interface.

        let mut methods = Vec::with_capacity(item_trait.items.len());
        let mut has_output = false;

        for (i, trait_item) in item_trait.items.iter_mut().enumerate() {
            match trait_item {
                TraitItem::Method(method) => {
                    methods.push(ServiceMethodDescriptor::try_from(i, method)?);
                }
                TraitItem::Type(ty) if ty.ident == "Output" => {
                    if !ty.bounds.is_empty() {
                        let msg = "`Output` type must not have bounds";
                        return Err(darling::Error::custom(msg).with_span(trait_item));
                    }
                    has_output = true;
                }
                _ => {
                    let msg = "Unsupported item in an Exonum interface";
                    return Err(darling::Error::custom(msg).with_span(trait_item));
                }
            }
        }

        if !has_output {
            item_trait.items.push(syn::parse_quote! {
                /// Type of items output by the stub.
                type Output;
            });
        }

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
            .map(String::as_str)
            .unwrap_or_default()
    }

    fn mut_trait_name(&self) -> Ident {
        let name = format!("{}Mut", self.item_trait.ident);
        Ident::new(&name, Span::call_site())
    }

    fn impl_interface(&self) -> impl ToTokens {
        let cr = &self.attrs.cr;
        let trait_name = &self.item_trait.ident;
        let interface_name = self.interface_name();

        let impl_match_arm = |descriptor: &ServiceMethodDescriptor| {
            let ServiceMethodDescriptor { name, arg_type, id } = descriptor;

            quote! {
                #id => {
                    let arg: #arg_type = #cr::merkledb::BinaryValue::from_bytes(payload.into())
                        .map_err(#cr::runtime::DispatcherError::malformed_arguments)?;
                    self.#name(context, arg)
                }
            }
        };
        let match_arms = self.methods.iter().map(impl_match_arm);

        let ctx = quote!(#cr::runtime::rust::CallContext<'a>);
        let res = quote!(std::result::Result<(), #cr::runtime::ExecutionError>);
        quote! {
            impl<'a> #cr::runtime::rust::Interface<'a> for dyn #trait_name<#ctx, Output = #res> {
                const INTERFACE_NAME: &'static str = #interface_name;

                fn dispatch(
                    &self,
                    context: #cr::runtime::rust::CallContext<'a>,
                    method: #cr::runtime::MethodId,
                    payload: &[u8],
                ) -> #res {
                    match method {
                        #( #match_arms )*
                        _ => Err(#cr::runtime::DispatcherError::NoSuchMethod.into()),
                    }
                }
            }
        }
    }

    fn impl_trait_for_generic_stub(&self) -> impl ToTokens {
        let cr = &self.attrs.cr;
        let trait_name = &self.item_trait.ident;
        let mut_trait_name = self.mut_trait_name();
        let interface_name = self.interface_name();

        let impl_method = |descriptor: &ServiceMethodDescriptor| {
            let ServiceMethodDescriptor { name, arg_type, id } = descriptor;
            let name_string = name.to_string();
            let descriptor = quote! {
                #cr::runtime::rust::MethodDescriptor::new(
                    #interface_name,
                    #name_string,
                    #id,
                )
            };

            let method = quote! {
                fn #name(&self, context: Ctx, arg: #arg_type) -> Self::Output {
                    #cr::runtime::rust::GenericCall::generic_call(
                        self,
                        context,
                        #descriptor,
                        #cr::merkledb::BinaryValue::into_bytes(arg),
                    )
                }
            };
            let mut_method = quote! {
                fn #name(&mut self, context: Ctx, arg: #arg_type) -> Self::Output {
                    #cr::runtime::rust::GenericCallMut::generic_call_mut(
                        self,
                        context,
                        #descriptor,
                        #cr::merkledb::BinaryValue::into_bytes(arg),
                    )
                }
            };
            (method, mut_method)
        };

        let (methods, mut_methods): (Vec<_>, Vec<_>) = self.methods.iter().map(impl_method).unzip();
        quote! {
            impl<Ctx, T: #cr::runtime::rust::GenericCall<Ctx>> #trait_name<Ctx> for T {
                type Output = <T as #cr::runtime::rust::GenericCall<Ctx>>::Output;
                #( #methods )*
            }

            impl<Ctx, T: #cr::runtime::rust::GenericCallMut<Ctx>> #mut_trait_name<Ctx> for T {
                type Output = <T as #cr::runtime::rust::GenericCallMut<Ctx>>::Output;
                #( #mut_methods )*
            }
        }
    }

    fn mut_trait(&self) -> impl ToTokens {
        let mut mut_trait = self.item_trait.clone();
        mut_trait.ident = self.mut_trait_name();

        for trait_item in &mut mut_trait.items {
            if let TraitItem::Method(method) = trait_item {
                if let FnArg::Receiver(ref mut recv) = method.sig.inputs[0] {
                    recv.mutability = Some(syn::parse_quote!(mut));
                }
            }
        }

        quote!(#mut_trait)
    }
}

impl ToTokens for ExonumService {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let item_trait = &self.item_trait;
        let mut_trait = self.mut_trait();
        let impl_interface = self.impl_interface();
        let impl_trait = self.impl_trait_for_generic_stub();

        let expanded = quote! {
            #mut_trait
            #item_trait
            #impl_trait
            #impl_interface
        };
        tokens.extend(expanded);
    }
}

pub fn impl_exonum_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_trait = parse_macro_input!(item as ItemTrait);
    let attrs = parse_macro_input!(attr as AttributeArgs);

    let exonum_service = match ExonumService::new(item_trait, attrs) {
        Ok(exonum_service) => exonum_service,
        Err(e) => return e.write_errors().into(),
    };
    let tokens = quote!(#exonum_service);
    tokens.into()
}
