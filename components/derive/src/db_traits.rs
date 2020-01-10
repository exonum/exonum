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

use darling::{ast::Fields, FromDeriveInput, FromField, FromMeta};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use syn::{spanned::Spanned, Data, DataStruct, DeriveInput, Generics};

use std::collections::HashSet;

use crate::find_meta_attrs;

#[derive(Debug)]
struct BinaryValueStruct {
    ident: Ident,
    attrs: BinaryValueAttrs,
}

impl FromDeriveInput for BinaryValueStruct {
    fn from_derive_input(input: &DeriveInput) -> darling::Result<Self> {
        let attrs = find_meta_attrs("binary_value", &input.attrs)
            .map(|meta| BinaryValueAttrs::from_nested_meta(&meta))
            .unwrap_or_else(|| Ok(BinaryValueAttrs::default()))?;

        Ok(Self {
            ident: input.ident.clone(),
            attrs,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Codec {
    Protobuf,
    Bincode,
}

impl Default for Codec {
    fn default() -> Self {
        Codec::Protobuf
    }
}

impl FromMeta for Codec {
    fn from_string(value: &str) -> darling::Result<Self> {
        match value {
            "protobuf" => Ok(Codec::Protobuf),
            "bincode" => Ok(Codec::Bincode),
            _ => {
                let msg = format!(
                    "Unknown codec ({}). Use one of `protobuf` or `bincode`",
                    value
                );
                Err(darling::Error::custom(msg))
            }
        }
    }
}

#[derive(Debug, Default, FromMeta)]
struct BinaryValueAttrs {
    #[darling(default)]
    codec: Codec,
}

#[derive(Debug, FromDeriveInput)]
struct ObjectHashStruct {
    ident: Ident,
}

impl ObjectHashStruct {
    pub fn implement_object_hash(&self) -> impl ToTokens {
        let name = &self.ident;

        quote! {
            impl exonum_merkledb::ObjectHash for #name {
                fn object_hash(&self) -> exonum_merkledb::_reexports::Hash {
                    let bytes = exonum_merkledb::BinaryValue::to_bytes(self);
                    exonum_merkledb::_reexports::hash(&bytes)
                }
            }
        }
    }
}

impl BinaryValueStruct {
    fn implement_binary_value_from_pb(&self) -> proc_macro2::TokenStream {
        let name = &self.ident;

        quote! {
            impl exonum_merkledb::BinaryValue for #name {
                fn to_bytes(&self) -> Vec<u8> {
                    use protobuf::Message as _;
                    // This trait assumes that we work with trusted data so we can unwrap here.
                    exonum_proto::ProtobufConvert::to_pb(self).write_to_bytes().expect(
                        concat!("Failed to serialize `BinaryValue` for ", stringify!(#name))
                    )
                }

                fn from_bytes(
                    value: std::borrow::Cow<[u8]>,
                ) -> std::result::Result<Self, exonum_merkledb::_reexports::Error> {
                    use protobuf::Message as _;

                    let mut block = <Self as exonum_proto::ProtobufConvert>::ProtoStruct::new();
                    block.merge_from_bytes(value.as_ref())?;
                    exonum_proto::ProtobufConvert::from_pb(block)
                }
            }
        }
    }

    fn implement_binary_value_from_bincode(&self) -> proc_macro2::TokenStream {
        let name = &self.ident;

        quote! {
            impl exonum_merkledb::BinaryValue for #name {
                fn to_bytes(&self) -> std::vec::Vec<u8> {
                    bincode::serialize(self).expect(
                        concat!("Failed to serialize `BinaryValue` for ", stringify!(#name))
                    )
                }

                fn from_bytes(
                    value: std::borrow::Cow<[u8]>,
                ) -> std::result::Result<Self, exonum_merkledb::_reexports::Error> {
                    bincode::deserialize(value.as_ref()).map_err(From::from)
                }
            }
        }
    }

    fn implement_binary_value(&self) -> impl ToTokens {
        match self.attrs.codec {
            Codec::Protobuf => self.implement_binary_value_from_pb(),
            Codec::Bincode => self.implement_binary_value_from_bincode(),
        }
    }
}

impl ToTokens for BinaryValueStruct {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mod_name = Ident::new(
            &format!("binary_value_impl_{}", self.ident),
            Span::call_site(),
        );

        let binary_value = self.implement_binary_value();
        let expanded = quote! {
            mod #mod_name {
                use super::*;
                #binary_value
            }
        };

        tokens.extend(expanded);
    }
}

impl ToTokens for ObjectHashStruct {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let object_hash = self.implement_object_hash();
        let expanded = quote! { #object_hash };

        tokens.extend(expanded);
    }
}

pub fn impl_binary_value(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let db_object = BinaryValueStruct::from_derive_input(&input)
        .unwrap_or_else(|e| panic!("BinaryValue: {}", e));
    let tokens = quote! { #db_object };
    tokens.into()
}

pub fn impl_object_hash(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let db_object =
        ObjectHashStruct::from_derive_input(&input).unwrap_or_else(|e| panic!("ObjectHash: {}", e));
    let tokens = quote! { #db_object };
    tokens.into()
}

/// Checks that an ASCII character is allowed in the `IndexAddress` component.
pub fn is_allowed_component_char(c: u8) -> bool {
    match c {
        b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' => true,
        _ => false,
    }
}

fn validate_address_component(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Name shouldn't be empty".to_owned());
    }
    if !name
        .as_bytes()
        .iter()
        .copied()
        .all(is_allowed_component_char)
    {
        return Err(format!(
            "Name `{}` contains invalid chars (allowed: `A-Z`, `a-z`, `0-9`, `_` and `-`)",
            name
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct FromAccess {
    ident: Ident,
    access_ident: Ident,
    fields: Vec<AccessField>,
    generics: Generics,
    attrs: FromAccessAttrs,
}

#[derive(Debug, Default, FromMeta)]
struct FromAccessAttrs {
    #[darling(default)]
    transparent: bool,
}

#[derive(Debug, Default, FromMeta)]
struct FromAccessFieldAttrs {
    #[darling(default)]
    rename: Option<String>,
    #[darling(default)]
    flatten: bool,
}

impl FromAccess {
    fn extract_access_ident(generics: &syn::Generics) -> darling::Result<&Ident> {
        use syn::{TraitBound, TypeParamBound};

        for type_param in generics.type_params() {
            if type_param
                .attrs
                .iter()
                .any(|attr| attr.path.is_ident("from_access"))
            {
                return Ok(&type_param.ident);
            }
        }

        for type_param in generics.type_params() {
            for bound in type_param.bounds.iter() {
                if let TypeParamBound::Trait(TraitBound { path, .. }) = bound {
                    if path.is_ident("Access") {
                        return Ok(&type_param.ident);
                    }
                }
            }
        }

        // No type params with the overt attribute or `T: Access` constraint.
        let mut params = generics.type_params();
        let type_param = params.next().ok_or_else(|| {
            darling::Error::custom("`FromAccess` struct should be generic over `Access` type")
        })?;
        if params.next().is_some() {
            let msg = "Cannot find type param implementing `Access` trait. \
                       You may mark it explicitly with `#[from_access]`";
            let e = darling::Error::custom(msg);
            Err(e)
        } else {
            // If there is a single type param, we hope it's the correct one.
            Ok(&type_param.ident)
        }
    }
}

impl FromDeriveInput for FromAccess {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        let attrs = find_meta_attrs("from_access", &input.attrs)
            .map(|meta| FromAccessAttrs::from_nested_meta(&meta))
            .unwrap_or_else(|| Ok(FromAccessAttrs::default()))?;

        match &input.data {
            Data::Struct(DataStruct { fields, .. }) => {
                let this = Self {
                    ident: input.ident.clone(),
                    access_ident: Self::extract_access_ident(&input.generics)?.clone(),
                    generics: input.generics.clone(),
                    fields: Fields::try_from(fields)?.fields,
                    attrs,
                };

                if this.attrs.transparent {
                    if this.fields.len() != 1 {
                        let e = darling::Error::custom(
                            "Transparent struct must contain a single field",
                        );
                        return Err(e);
                    }
                } else {
                    let mut field_names = HashSet::new();

                    for field in &this.fields {
                        if let Some(ref name) = field.name_suffix {
                            validate_address_component(name).map_err(|msg| {
                                darling::Error::custom(msg).with_span(&field.span)
                            })?;
                            if !field_names.insert(name) {
                                let e = "Duplicate field name";
                                return Err(darling::Error::custom(e).with_span(&field.span));
                            }
                        } else if !field.flatten {
                            let msg = if this.fields.len() == 1 {
                                "Unnamed fields necessitate #[from_access(rename = ...)]. \
                                 To use a wrapper, add #[from_access(transparent)] to the struct"
                            } else {
                                "Unnamed fields necessitate #[from_access(rename = ...)]"
                            };
                            let e = darling::Error::custom(msg).with_span(&field.span);
                            return Err(e);
                        }
                    }
                }
                Ok(this)
            }
            _ => Err(darling::Error::unsupported_shape(
                "`FromAccess` can be only implemented for structs",
            )),
        }
    }
}

#[derive(Debug)]
struct AccessField {
    span: Span,
    ident: Option<Ident>,
    name_suffix: Option<String>,
    flatten: bool,
}

impl FromField for AccessField {
    fn from_field(field: &syn::Field) -> darling::Result<Self> {
        let ident = field.ident.clone();

        let attrs = find_meta_attrs("from_access", &field.attrs)
            .map(|meta| FromAccessFieldAttrs::from_nested_meta(&meta))
            .unwrap_or_else(|| Ok(FromAccessFieldAttrs::default()))?;

        let name_suffix = attrs
            .rename
            .or_else(|| ident.as_ref().map(ToString::to_string));
        Ok(Self {
            ident,
            name_suffix,
            span: field.span(),
            flatten: attrs.flatten,
        })
    }
}

impl AccessField {
    fn ident(&self, field_index: usize) -> impl ToTokens {
        if let Some(ref ident) = self.ident {
            quote!(#ident)
        } else {
            let field_index = syn::Index::from(field_index);
            quote!(#field_index)
        }
    }

    fn constructor(&self, field_index: usize) -> impl ToTokens {
        let from_access = quote!(exonum_merkledb::access::FromAccess);
        let ident = self.ident(field_index);
        if self.flatten {
            quote!(#ident: #from_access::from_access(access.clone(), addr.clone())?)
        } else {
            let name = self.name_suffix.as_ref().unwrap();
            quote!(#ident: #from_access::from_access(access.clone(), addr.clone().append_name(#name))?)
        }
    }

    fn root_constructor(&self, field_index: usize) -> impl ToTokens {
        let from_access = quote!(exonum_merkledb::access::FromAccess);
        let ident = self.ident(field_index);
        if self.flatten {
            quote!(#ident: #from_access::from_root(access.clone())?)
        } else {
            let name = &self.name_suffix;
            quote!(#ident: #from_access::from_access(access.clone(), #name.into())?)
        }
    }
}

impl FromAccess {
    fn access_fn(&self) -> impl ToTokens {
        let fn_impl = if self.attrs.transparent {
            let from_access = quote!(exonum_merkledb::access::FromAccess);
            let ident = self.fields[0].ident(0);
            quote!(Ok(Self { #ident: #from_access::from_access(access, addr)? }))
        } else {
            let field_constructors = self
                .fields
                .iter()
                .enumerate()
                .map(|(i, field)| field.constructor(i));
            quote!(Ok(Self { #(#field_constructors,)* }))
        };

        let access_ident = &self.access_ident;
        quote! {
            fn from_access(
                access: #access_ident,
                addr: exonum_merkledb::IndexAddress,
            ) -> Result<Self, exonum_merkledb::access::AccessError> {
                #fn_impl
            }
        }
    }

    fn root_fn(&self) -> impl ToTokens {
        let fn_impl = if self.attrs.transparent {
            let from_access = quote!(exonum_merkledb::access::FromAccess);
            let ident = self.fields[0].ident(0);
            quote!(Ok(Self { #ident: #from_access::from_root(access)? }))
        } else {
            let field_constructors = self
                .fields
                .iter()
                .enumerate()
                .map(|(i, field)| field.root_constructor(i));
            quote!(Ok(Self { #(#field_constructors,)* }))
        };

        let access_ident = &self.access_ident;
        quote! {
            fn from_root(
                access: #access_ident,
            ) -> Result<Self, exonum_merkledb::access::AccessError> {
                #fn_impl
            }
        }
    }
}

impl ToTokens for FromAccess {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.ident;
        let tr = quote!(exonum_merkledb::access::FromAccess);
        let access_ident = &self.access_ident;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();

        let from_access_fn = self.access_fn();
        let from_root_fn = self.root_fn();

        let expanded = quote! {
            impl #impl_generics #tr<#access_ident> for #name #ty_generics #where_clause {
                #from_access_fn
                #from_root_fn
            }
        };
        tokens.extend(expanded);
    }
}

pub fn impl_from_access(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();
    let from_access = match FromAccess::from_derive_input(&input) {
        Ok(access) => access,
        Err(e) => return e.write_errors().into(),
    };
    let tokens = quote!(#from_access);
    tokens.into()
}
