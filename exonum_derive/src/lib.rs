// Copyright 2018 The Exonum Team
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

//! Macros.

#![recursion_limit = "256"]

extern crate proc_macro;
extern crate syn;

#[macro_use] extern crate synstructure;
#[macro_use] extern crate quote;

use syn::{Fields, FieldsUnnamed};
use synstructure::{BindStyle, Structure, VariantInfo};

decl_derive!([TransactionSet] => transaction_set_derive);

fn transaction_set_derive(mut s: Structure) -> quote::Tokens {
    let tx_set = impl_transaction_set(&s);
    let into_box = impl_into_box(&mut s);
    let deserialize = impl_deserialize(&s);
    let serialize = impl_serialize(&mut s);

    quote!{
        #tx_set
        #into_box
        #deserialize
        #serialize
    }
}

/// Implements `TransactionSet`.
fn impl_transaction_set(s: &Structure) -> quote::Tokens {
    let match_body = s.variants().iter().fold(quote!(), |acc, v| {
        let tx_type = tx_type(v);
        let constructor = v.construct(|_, _| quote!(tx));
        let match_hand = quote! {
            <#tx_type as ::exonum::messages::ServiceMessage>::MESSAGE_ID => {
                let tx = ::exonum::messages::Message::from_raw(raw)?;
                Ok(#constructor)
            }
        };
        quote!(#acc #match_hand)
    });

    s.unbound_impl(
        quote!(::exonum::blockchain::TransactionSet),
        quote! {
            fn tx_from_raw(
                raw: ::exonum::messages::RawTransaction
            ) -> ::std::result::Result<Self, ::exonum::encoding::Error> {
                let message_type = raw.message_type();
                match message_type {
                    #match_body
                    _ => return Err(::exonum::encoding::Error::IncorrectMessageType {
                        message_type,
                    })
                }
            }
        }
    )
}

/// Implements `Into<Box<Transaction>>`.
fn impl_into_box(s: &mut Structure) -> quote::Tokens {
    for v in s.variants_mut() {
        v.bind_with(|_| BindStyle::Move);
    }
    let match_body = s.each_variant(|v| {
        let ident = &v.bindings()[0].binding;
        quote!(::std::boxed::Box::new(#ident))
    });

    s.unbound_impl(
        quote!(::std::convert::Into<::std::boxed::Box<::exonum::blockchain::Transaction>>),
        quote! {
            fn into(self) -> ::std::boxed::Box<::exonum::blockchain::Transaction> {
                match self {
                    #match_body,
                }
            }
        },
    )
}

/// Implements `serde::DeserializeOwned`.
fn impl_deserialize(s: &Structure) -> quote::Tokens {
    let name = &s.ast().ident;

    let match_body = s.variants().iter().fold(quote!(), |acc, v| {
        let tx_type = tx_type(v);
        let variant_name = &v.ast().ident;

        let match_hand = quote! {
            <#tx_type as ::exonum::messages::ServiceMessage>::MESSAGE_ID => {
                <#tx_type as ::exonum::encoding::serialize::json::ExonumJsonDeserialize>
                    ::deserialize(&value)
                    .map_err(|e| D::Error::custom(
                        format!("Can't deserialize a value: {}", e.description())
                    ))
                    .map(#name::#variant_name)
            }
        };
        quote!(#acc #match_hand)
    });

    quote! {
        impl<'de> ::exonum::encoding::serialize::reexport::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: ::exonum::encoding::serialize::reexport::Deserializer<'de>,
            {
                use ::exonum::encoding::serialize::json::reexport::{Value, from_value};
                use ::exonum::encoding::serialize::reexport::{DeError, Deserialize};

                let value = <Value as Deserialize>::deserialize(deserializer)?;
                let message_id: Value = value.get("message_id")
                    .ok_or(D::Error::custom("Can't get message_id from json"))?
                    .clone();
                let message_id: u16 = from_value(message_id)
                    .map_err(|e| D::Error::custom(
                        format!("Can't deserialize message_id: {}", e)
                    ))?;

                match message_id {
                    #match_body
                    _ => Err(D::Error::custom(format!("invalid message_id: {}", message_id))),
                }
            }
        }
    }
}

/// Implements `serde::Serialize`.
fn impl_serialize(s: &mut Structure) -> quote::Tokens {
    for v in s.variants_mut() {
        v.bind_with(|_| BindStyle::Ref);
    }
    let match_body = s.each_variant(|v| {
        let ident = &v.bindings()[0].binding;
        quote!(Serialize::serialize(#ident, serializer))
    });

    s.unbound_impl(
        quote!(::exonum::encoding::serialize::reexport::Serialize),
        quote! {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::exonum::encoding::serialize::reexport::Serializer,
            {
                use ::exonum::encoding::serialize::reexport::Serialize;

                match *self {
                    #match_body
                }
            }
        }
    )
}

fn tx_type<'a, 'r: 'a>(variant: &'r VariantInfo<'a>) -> &'r syn::Type {
    match *variant.ast().fields {
        Fields::Unnamed(FieldsUnnamed { ref unnamed, .. }) => {
            assert_eq!(unnamed.len(), 1, "Incorrect enum variant");
            let field = unnamed.first().unwrap();
            &field.value().ty
        }
        _ => panic!("Incorrect enum variant"),
    }
}
