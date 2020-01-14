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

use darling::{self, FromMeta};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, spanned::Spanned, Attribute, AttributeArgs, FnArg, Ident, ItemTrait,
    NestedMeta, Receiver, ReturnType, TraitItem, TraitItemMethod, Type,
};

use std::collections::HashSet;
use std::convert::TryFrom;

use super::{find_meta_attrs, RustRuntimeCratePath};

#[derive(Debug)]
struct ServiceMethodDescriptor {
    name: Ident,
    arg_type: Box<Type>,
    id: u32,
}

const INVALID_METHOD_MSG: &str =
    "Interface method should have form `fn foo(&self, ctx: Ctx, arg: Bar) -> Self::Output`";

fn invalid_method(span: &impl Spanned) -> darling::Error {
    darling::Error::custom(INVALID_METHOD_MSG).with_span(span)
}

impl ServiceMethodDescriptor {
    /// Tries to parse a method definition from its declaration in the trait. The method needs
    /// to correspond to the following form:
    ///
    /// ```text
    /// fn foo(&self, ctx: Ctx, arg: Bar) -> Self::Output;
    /// ```
    ///
    /// where `Ctx` is the context type param defined in the trait.
    fn try_from(
        method_id: u32,
        ctx: &Ident,
        method: &TraitItemMethod,
    ) -> Result<Self, darling::Error> {
        use syn::{PatType, TypePath};

        let mut method_args_iter = method.sig.inputs.iter();

        // Check the validity of the method receiver (should be `&self`).
        if let Some(arg) = method_args_iter.next() {
            match arg {
                FnArg::Receiver(Receiver {
                    reference: Some(_),
                    mutability: None,
                    ..
                }) => {}
                _ => {
                    return Err(invalid_method(&arg));
                }
            }
        } else {
            return Err(invalid_method(method));
        }

        // Check the validity of the first arg, excluding the receiver (should be
        // the context type param).
        let ctx_type = method_args_iter
            .next()
            .ok_or_else(|| invalid_method(method))?;
        if let FnArg::Typed(PatType { ty, .. }) = ctx_type {
            if let Type::Path(TypePath { path, .. }) = ty.as_ref() {
                if path.get_ident() != Some(ctx) {
                    // Invalid argument type.
                    return Err(invalid_method(path));
                }
            } else {
                // Type is not path-like.
                return Err(invalid_method(ty));
            }
        } else {
            // Not a typed argument.
            return Err(invalid_method(ctx_type));
        }

        // Check the validity of the second arg (excluding the receiver) and extract the type
        // from it.
        let arg_type = method_args_iter
            .next()
            .ok_or_else(|| invalid_method(method))
            .and_then(|arg| match arg {
                FnArg::Typed(arg) => Ok(arg.ty.clone()),
                _ => Err(invalid_method(method)),
            })?;

        if method_args_iter.next().is_some() {
            return Err(invalid_method(method));
        }

        // Check the validity of the return type (should be `Self::Output`).
        if let ReturnType::Type(_, ref ty) = method.sig.output {
            if let Type::Path(type_path) = ty.as_ref() {
                let segments = &type_path.path.segments;
                if segments.len() == 2
                    && segments[0].ident == "Self"
                    && segments[1].ident == "Output"
                {
                    // Seems about right.
                } else {
                    // Invalid `type_path`.
                    return Err(invalid_method(segments));
                }
            } else {
                // Invalid return type format.
                return Err(invalid_method(ty));
            }
        } else {
            // "Default" return type (i.e., `()`).
            return Err(invalid_method(&method.sig));
        }

        Ok(ServiceMethodDescriptor {
            name: method.sig.ident.clone(),
            id: method_id, // TODO: allow to parse `method_id` from attrs
            arg_type,
        })
    }
}

#[derive(Debug, FromMeta)]
#[darling(default)]
struct ExonumServiceAttrs {
    #[darling(rename = "crate")]
    cr: RustRuntimeCratePath,
    interface: Option<String>,
}

impl Default for ExonumServiceAttrs {
    fn default() -> Self {
        Self {
            cr: RustRuntimeCratePath::default(),
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

#[derive(Debug, FromMeta)]
struct MethodIdAttr {
    id: u32,
}

impl TryFrom<&[Attribute]> for MethodIdAttr {
    type Error = darling::Error;

    fn try_from(args: &[Attribute]) -> Result<Self, Self::Error> {
        find_meta_attrs("interface_method", args)
            .map(|meta| Self::from_nested_meta(&meta))
            .unwrap_or_else(|| {
                let msg = format!(
                    "Unable to find method ID mapping for method. \
                     It should be specified, e.g. `#[interface_method(id = 0)]`",
                );
                Err(darling::Error::custom(msg))
            })
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
        use syn::GenericParam;

        // Extract context type param from the trait generics.
        let params = &item_trait.generics.params;
        let ctx_ident = if params.is_empty() {
            let msg = "Interface trait needs to have context type param";
            return Err(darling::Error::custom(msg).with_span(&item_trait.ident));
        } else if params.len() > 1 {
            let msg = "Multiple generics are not yet supported";
            return Err(darling::Error::custom(msg).with_span(params));
        } else if let GenericParam::Type(ref type_param) = params[0] {
            &type_param.ident
        } else {
            let msg = "Unsupported generic parameter (should be a type parameter denoting \
                       execution context)";
            return Err(darling::Error::custom(msg).with_span(&params[0]));
        };

        // Process trait methods.
        let mut methods = Vec::with_capacity(item_trait.items.len());
        let mut has_output = false;
        let mut used_method_ids = HashSet::new();

        for trait_item in &item_trait.items {
            match trait_item {
                TraitItem::Method(method) => {
                    let id_attr = MethodIdAttr::try_from(method.attrs.as_ref())?;
                    let method_id = id_attr.id;

                    if !used_method_ids.insert(method_id) {
                        let msg = format!("Method ID {} is already used", method_id);
                        return Err(darling::Error::custom(msg).with_span(&method.sig));
                    }

                    let method = ServiceMethodDescriptor::try_from(method_id, ctx_ident, method)?;
                    methods.push(method);
                }
                TraitItem::Type(ty) if ty.ident == "Output" => {
                    if !ty.bounds.is_empty() {
                        let msg = "`Output` type must not have bounds";
                        return Err(darling::Error::custom(msg).with_span(ty));
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
            let msg = "The trait should have associated `Output` type";
            return Err(darling::Error::custom(msg).with_span(&item_trait));
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

    /// Generates `Interface` implementation for the trait object with matching params
    /// (`CallContext` context and `Result<(), ExecutionError>` output). This will allow to call
    /// implementation methods from the dispatcher.
    fn impl_interface(&self) -> impl ToTokens {
        let cr = &self.attrs.cr;
        let trait_name = &self.item_trait.ident;
        let interface_name = self.interface_name();

        let impl_match_arm = |descriptor: &ServiceMethodDescriptor| {
            let ServiceMethodDescriptor { name, arg_type, id } = descriptor;

            quote! {
                #id => {
                    let arg: #arg_type = exonum::merkledb::BinaryValue::from_bytes(payload.into())
                        .map_err(exonum::runtime::CommonError::malformed_arguments)?;
                    self.#name(context, arg)
                }
            }
        };
        let match_arms = self.methods.iter().map(impl_match_arm);

        let ctx = quote!(#cr::CallContext<'a>);
        let res = quote!(std::result::Result<(), exonum::runtime::ExecutionError>);
        quote! {
            impl<'a> #cr::Interface<'a> for dyn #trait_name<#ctx, Output = #res> {
                const INTERFACE_NAME: &'static str = #interface_name;

                fn dispatch(
                    &self,
                    context: #cr::CallContext<'a>,
                    method: exonum::runtime::MethodId,
                    payload: &[u8],
                ) -> #res {
                    match method {
                        #( #match_arms )*
                        _ => Err(exonum::runtime::CommonError::NoSuchMethod.into()),
                    }
                }
            }
        }
    }

    /// Implements the user trait for any type implementing low-level stubs (`GenericCall` /
    /// `GenericCallMut`). This means that the trait is implemented for all stub implementations
    /// (such as keypairs) with zero dedicated code.
    fn impl_trait_for_generic_stub(&self) -> impl ToTokens {
        let cr = &self.attrs.cr;
        let trait_name = &self.item_trait.ident;
        let mut_trait_name = self.mut_trait_name();
        let interface_name = self.interface_name();

        let impl_method = |descriptor: &ServiceMethodDescriptor| {
            let ServiceMethodDescriptor { name, arg_type, id } = descriptor;
            let name_string = name.to_string();
            let descriptor = quote! {
                #cr::MethodDescriptor::new(
                    #interface_name,
                    #name_string,
                    #id,
                )
            };

            let method = quote! {
                fn #name(&self, context: Ctx, arg: #arg_type) -> Self::Output {
                    #cr::GenericCall::generic_call(
                        self,
                        context,
                        #descriptor,
                        exonum::merkledb::BinaryValue::into_bytes(arg),
                    )
                }
            };
            let mut_method = quote! {
                fn #name(&mut self, context: Ctx, arg: #arg_type) -> Self::Output {
                    #cr::GenericCallMut::generic_call_mut(
                        self,
                        context,
                        #descriptor,
                        exonum::merkledb::BinaryValue::into_bytes(arg),
                    )
                }
            };
            (method, mut_method)
        };

        let (methods, mut_methods): (Vec<_>, Vec<_>) = self.methods.iter().map(impl_method).unzip();
        // Since `Ctx` type param is defined by our code, it doesn't have to correspond to the name
        // chosen by the user.
        quote! {
            impl<Ctx, T: #cr::GenericCall<Ctx>> #trait_name<Ctx> for T {
                type Output = <T as #cr::GenericCall<Ctx>>::Output;
                #( #methods )*
            }

            impl<Ctx, T: #cr::GenericCallMut<Ctx>> #mut_trait_name<Ctx> for T {
                type Output = <T as #cr::GenericCallMut<Ctx>>::Output;
                #( #mut_methods )*
            }
        }
    }

    /// Creates a mutable version of the trait by appending `Mut` to the trait name and changing
    /// `&self` receivers in the trait methods to `&mut self`. No other changes are performed.
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
