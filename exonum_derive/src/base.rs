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

//! `MessageSet` derivation.

use quote;
use syn::{Ident, Lifetime};
use synstructure::{BindStyle, Structure, VariantInfo};

use structure::Payload;
use utils::{execute_shifts, strip_lifetimes};

pub fn base_derive(s: Structure) -> quote::Tokens {
    let s = Payload::try_from(s).unwrap();

    let ids_enum = impl_message_ids_enum(&s);
    let message_set = impl_message_set(&s);
    let read = impl_read(&s);
    let write = impl_write(&s);
    let serialize = impl_serialize(&s);
    let json_deserialize = impl_json_deserialize(&s);

    quote!(
        #message_set
        #read
        #write
        #serialize
        #json_deserialize
        #ids_enum
    )
}

fn impl_message_ids_enum(s: &Payload) -> quote::Tokens {
    let variants = s.variants().iter().enumerate().map(|(i, variant)| {
        let i = i as u16;
        let name = variant.ast().ident;
        let doccomment =
            format!(
            "Message identifier for `{}::{}` transactions.",
            s.ast().ident.as_ref(),
            name.as_ref(),
        );

        quote!(
            #[doc = #doccomment]
            #name = #i,
        )
    });

    let ident = &s.ids_enum_ident;
    let header_size_fn = header_size_fn(s);

    quote!(
        /// Message identifiers.
        #[repr(u16)]
        #[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Ord, Eq, Hash)]
        pub enum #ident {
            #(#variants)*
        }

        impl ::std::convert::From<#ident> for u16 {
            fn from(value: #ident) -> u16 {
                value as u16
            }
        }

        impl ::exonum::encoding::MeasureHeader for #ident {
            #header_size_fn
        }
    )
}

#[test]
fn test_impl_message_ids_enum() {
    let input = parse_quote!(
        #[exonum(service_id = "10")]
        enum Transaction<'a> {
            Create { public_key: &'a PublicKey, name: &'a str },
            Transfer { from: &'a PublicKey, to: &'a PublicKey, amount: u64 }
        }
    );
    let s = Payload::try_from(Structure::new(&input)).unwrap();
    // Tested separately, so we don't bother here.
    let header_size_fn = header_size_fn(&s);

    assert_eq!(
        impl_message_ids_enum(&s),
        quote!(
            /// Message identifiers.
            #[repr(u16)]
            #[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Ord, Eq, Hash)]
            pub enum TransactionIds {
                #[doc = "Message identifier for `Transaction::Create` transactions."]
                Create = 0u16,
                #[doc = "Message identifier for `Transaction::Transfer` transactions."]
                Transfer = 1u16,
            }

            impl ::std::convert::From<TransactionIds> for u16 {
                fn from(value: TransactionIds) -> u16 {
                    value as u16
                }
            }

            impl ::exonum::encoding::MeasureHeader for TransactionIds {
                #header_size_fn
            }
        )
    );
}

fn write_for_variant(variant: &VariantInfo) -> quote::Tokens {
    let code = execute_shifts(&variant, |_, binding| {
        let field_name = &binding.binding;
        quote!(
            writer.write_ref(#field_name, __from, __from + __size);
        )
    });
    let pat = variant.pat();
    quote!(#pat => { #code })
}

#[test]
fn test_write_for_variant() {
    let input = parse_quote!(
        enum Transaction<'a> {
            Create { public_key: &'a PublicKey, name: &'a str },
            Transfer { from: &'a PublicKey, to: &'a PublicKey, amount: u64 }
        }
    );
    let s = Structure::new(&input);

    assert_eq!(
        write_for_variant(&s.variants()[0]),
        quote!(
            Transaction::Create {
                public_key: ref __binding_0,
                name: ref __binding_1,
            } => {
                let __from = 0 as ::exonum::encoding::Offset;
                let __size = <&PublicKey as ::exonum::encoding::Field>::field_size();
                writer.write_ref(__binding_0, __from, __from + __size);
                let __from = __from + __size;
                let __size = <&str as ::exonum::encoding::Field>::field_size();
                writer.write_ref(__binding_1, __from, __from + __size);
            }
        )
    );
}

fn header_size_fn(s: &Payload) -> quote::Tokens {
    let ids_enum = &s.ids_enum_ident;

    let match_hands = s.variants().iter().map(|variant| {
        let mut first = true;

        let size = variant.bindings().iter().map(|b| &b.ast().ty).fold(
            quote!(),
            |acc, ty| {
                let maybe_plus = if first {
                    first = false;
                    quote!()
                } else {
                    quote!(+)
                };

                let ty = strip_lifetimes(ty);
                quote!(#acc #maybe_plus <#ty as ::exonum::encoding::Field>::field_size())
            },
        );

        let ident = &variant.ast().ident;
        quote!(#ids_enum::#ident => { #size })
    });

    quote!(
        fn header_size(&self) -> ::exonum::encoding::Offset {
            match *self {
                #(#match_hands)*
            }
        }
    )
}

#[test]
fn test_header_size_fn() {
    let input = parse_quote!(
        #[exonum(service_id = "10")]
        enum Transaction<'a> {
            Create { public_key: &'a PublicKey, name: &'a str },
            Transfer { from: &'a PublicKey, to: &'a PublicKey, amount: u64 }
        }
    );
    let s = Payload::try_from(Structure::new(&input)).unwrap();

    assert_eq!(
        header_size_fn(&s),
        quote!(
            fn header_size(&self) -> ::exonum::encoding::Offset {
                match *self {
                    TransactionIds::Create => {
                        <&PublicKey as ::exonum::encoding::Field>::field_size() +
                            <&str as ::exonum::encoding::Field>::field_size()
                    }
                    TransactionIds::Transfer => {
                        <&PublicKey as ::exonum::encoding::Field>::field_size() +
                            <&PublicKey as ::exonum::encoding::Field>::field_size() +
                            <u64 as ::exonum::encoding::Field>::field_size()
                    }
                }
            }
        )
    );
}

fn header_size_matches(s: &Payload) -> quote::Tokens {
    let ids_enum = &s.ids_enum_ident;

    let matches = s.variants().iter().enumerate().map(|(i, variant)| {
        let ident = &variant.ast().ident;
        let pat = s.id_pat(i);

        quote!(
            #pat => ::exonum::encoding::MeasureHeader::header_size(&#ids_enum::#ident),
        )
    });

    // FIXME incorrect `message_type` in error
    quote!(
        #(#matches)*
        _ => {
            return Err(::exonum::encoding::Error::IncorrectMessageType {
                message_type: 0,
            });
        }
    )
}

#[test]
fn test_header_size_matches() {
    let input = parse_quote!(
        #[exonum(service_id = "10")]
        enum Transaction<'a> {
            Create { public_key: &'a PublicKey, name: &'a str },
            Transfer { from: &'a PublicKey, to: &'a PublicKey, amount: u64 }
        }
    );
    let s = Payload::try_from(Structure::new(&input)).unwrap();

    assert_eq!(
        header_size_matches(&s),
        quote!(
            x if x == TransactionIds::Create as u16 =>
                ::exonum::encoding::MeasureHeader::header_size(&TransactionIds::Create),
            x if x == TransactionIds::Transfer as u16 =>
                ::exonum::encoding::MeasureHeader::header_size(&TransactionIds::Transfer),
            _ => {
                return Err(::exonum::encoding::Error::IncorrectMessageType {
                    message_type: 0,
                });
            }
        )
    );
}

fn message_id_fn(s: &Payload) -> quote::Tokens {
    let ids_enum = &s.ids_enum_ident;

    let match_hands = s.variants().iter().map(|variant| {
        let mut variant = variant.clone();
        variant.filter(|_| false);
        let pat = variant.pat();
        let ident = &variant.ast().ident;
        quote!(#pat => #ids_enum::#ident,)
    });

    quote!(
        fn message_id(&self) -> #ids_enum {
            match *self {
                #(#match_hands)*
            }
        }
    )
}

#[test]
fn test_message_id_fn() {
    let input = parse_quote!(
        #[exonum(service_id = "10")]
        enum Transaction<'a> {
            Create { public_key: &'a PublicKey, name: &'a str },
            Transfer { from: &'a PublicKey, to: &'a PublicKey, amount: u64 }
        }
    );
    let s = Payload::try_from(Structure::new(&input)).unwrap();

    assert_eq!(
        message_id_fn(&s),
        quote!(
            fn message_id(&self) -> TransactionIds {
                match *self {
                    Transaction::Create { .. } => TransactionIds::Create,
                    Transaction::Transfer { .. } => TransactionIds::Transfer,
                }
            }
        )
    );
}

fn check_fields_code(variant: &VariantInfo) -> quote::Tokens {
    let initializer = quote!(
        let latest_segment = (
            header_size +
            ::exonum::messages::HEADER_LENGTH as ::exonum::encoding::Offset
        ).into();
    );

    let bindings_len = variant.bindings().len();
    let code = execute_shifts(variant, |i, binding| {
        let ty = strip_lifetimes(&binding.ast().ty);

        let latest_segment = quote!(
            raw.check::<#ty>(
                __from.into(),
                (__from + __size).into(),
                latest_segment,
            )?
        );

        if i == bindings_len - 1 {
            quote!(#latest_segment)
        } else {
            quote!(let latest_segment = #latest_segment;)
        }
    });

    quote!(#initializer #code)
}

#[test]
fn test_check_fields_code() {
    let input = parse_quote!(
        enum Transaction<'a> {
            Create { public_key: &'a PublicKey, name: &'a str },
            Transfer { from: &'a PublicKey, to: &'a PublicKey, amount: u64 }
        }
    );
    let s = Structure::new(&input);

    assert_eq!(
        check_fields_code(&s.variants()[0]),
        quote!(
            let latest_segment = (
                header_size +
                ::exonum::messages::HEADER_LENGTH as ::exonum::encoding::Offset
            ).into();

            let __from = 0 as ::exonum::encoding::Offset;
            let __size = <&PublicKey as ::exonum::encoding::Field>::field_size();
            let latest_segment = raw.check::<&PublicKey>(
                __from.into(),
                (__from + __size).into(),
                latest_segment,
            )?;

            let __from = __from + __size;
            let __size = <&str as ::exonum::encoding::Field>::field_size();
            raw.check::<&str>(
                __from.into(),
                (__from + __size).into(),
                latest_segment,
            )?
        )
    );
}

fn check_fn(s: &Payload) -> quote::Tokens {
    let universal_checks = quote!(
        if raw.version() != ::exonum::messages::PROTOCOL_MAJOR_VERSION {
            return Err(::exonum::encoding::Error::UnsupportedProtocolVersion {
                version: ::exonum::messages::PROTOCOL_MAJOR_VERSION
            });
        }

        if raw.network_id() != ::exonum::messages::TEST_NETWORK_ID {
            return Err(::exonum::encoding::Error::IncorrectNetworkId {
                network_id: ::exonum::messages::TEST_NETWORK_ID
            });
        }

        if raw.service_id() != <Self as ::exonum::messages::MessageSet>::SERVICE_ID {
            return Err(::exonum::encoding::Error::IncorrectServiceId {
                service_id: <Self as ::exonum::messages::MessageSet>::SERVICE_ID
            });
        }
    );

    let header_size_matches = header_size_matches(s);

    let len_check = quote!(
        let header_size = match raw.message_type() {
            #header_size_matches
        };

        let min_message_size = header_size as usize +
            ::exonum::messages::HEADER_LENGTH as usize +
            ::exonum::crypto::SIGNATURE_LENGTH as usize;
        if raw.len() < min_message_size {
            return Err(::exonum::encoding::Error::UnexpectedlyShortPayload {
                actual_size: raw.len() as ::exonum::encoding::Offset,
                minimum_size: min_message_size as ::exonum::encoding::Offset,
            });
        }
    );

    let check_fields_hands = s.variants().iter().enumerate().map(|(i, variant)| {
        let code = check_fields_code(variant);
        let pat = s.id_pat(i);
        quote!(#pat => { #code })
    });

    quote!(
        fn check(raw: &::exonum::messages::RawMessage)
            -> ::std::result::Result<(), ::exonum::encoding::Error>
        {
            #universal_checks
            #len_check

            let body_len = match raw.message_type() {
                #(#check_fields_hands)*
                _ => unreachable!(),
            };
            if body_len.unchecked_offset() as usize +
                ::exonum::crypto::SIGNATURE_LENGTH as usize != raw.len()  {
                return Err("Incorrect raw message length.".into())
            }

            Ok(())
        }
    )
}

fn impl_message_set(s: &Payload) -> quote::Tokens {
    let ids_enum = &s.ids_enum_ident;
    let service_id = &s.service_id;
    let message_id_fn = message_id_fn(s);

    s.unbound_impl(
        quote!(::exonum::messages::MessageSet),
        quote!(
            const SERVICE_ID: u16 = #service_id;
            type MessageId = #ids_enum;

            #message_id_fn
        ),
    )
}

fn unsafe_read_for_variant(variant: &VariantInfo) -> quote::Tokens {
    let code = execute_shifts(variant, |i, binding| {
        let field_type = &binding.ast().ty;
        let binding_name = Ident::from(format!("__binding_{}", i));

        quote!(
            let #binding_name = raw.read::<#field_type>(__from, __from + __size);
        )
    });

    let constructor = variant.construct(|_, i| Ident::from(format!("__binding_{}", i)));

    quote!(#code #constructor)
}

#[test]
fn test_unsafe_read_for_variant() {
    let input = parse_quote!(
        enum Transaction<'a> {
            Create { public_key: &'a PublicKey, name: &'a str },
            Transfer { from: &'a PublicKey, to: &'a PublicKey, amount: u64 }
        }
    );
    let s = Structure::new(&input);

    assert_eq!(
        unsafe_read_for_variant(&s.variants()[0]),
        quote!(
            let __from = 0 as ::exonum::encoding::Offset;
            let __size = <&PublicKey as ::exonum::encoding::Field>::field_size();
            let __binding_0 = raw.read::<&'a PublicKey>(__from, __from + __size);
            let __from = __from + __size;
            let __size = <&str as ::exonum::encoding::Field>::field_size();
            let __binding_1 = raw.read::<&'a str>(__from, __from + __size);

            Transaction::Create {
                public_key: __binding_0,
                name: __binding_1,
            }
        )
    );
}

fn impl_read(s: &Payload) -> quote::Tokens {
    use proc_macro2::{Span, Term};

    let check_fn = check_fn(s);
    let check = s.unbound_impl(quote!(::exonum::messages::Check), quote!(#check_fn));

    let match_hands = s.variants().iter().enumerate().map(|(i, variant)| {
        let code = unsafe_read_for_variant(variant);
        let pat = s.id_pat(i);
        quote!(#pat => { #code })
    });

    let default_lifetime = Lifetime::new(Term::intern("'a"), Span::call_site());
    let lifetime = s.lifetime.as_ref().unwrap_or(&default_lifetime);

    let read = s.unbound_impl(
        quote!(::exonum::messages::Read<#lifetime>),
        quote!(
            unsafe fn unchecked_read(raw: &#lifetime ::exonum::messages::RawMessage) -> Self {
                match raw.message_type() {
                    #(#match_hands)*
                    _ => unreachable!("unchecked_read used incorrectly"),
                }
            }
        ),
    );

    quote!(#check #read)
}

fn impl_write(s: &Payload) -> quote::Tokens {
    let write_payload_hands = s.variants().iter().map(write_for_variant);

    s.unbound_impl(
        quote!(::exonum::messages::Write<::exonum::messages::RawMessage>),
        quote!(
            fn write_payload(&self, writer: &mut ::exonum::messages::MessageWriter) {
                match *self {
                    #(#write_payload_hands)*
                }
            }
        ),
    )
}

fn serialize_for_variant(variant: &VariantInfo) -> quote::Tokens {
    let ident = &variant.ast().ident.to_string();
    let fields_number = variant.bindings().len();
    let bindings = variant.bindings();
    let field_name_strs = variant.bindings().iter().map(|b| {
        b.ast().ident.as_ref().unwrap().to_string()
    });

    let code = quote!(
        let mut value = serializer.serialize_struct(#ident, #fields_number)?;
        #(
            value.serialize_field(
                #field_name_strs,
                &#bindings.serialize_field().map_err(S::Error::custom)?,
            )?;
        )*
        value.end()
    );

    let pat = variant.pat();
    quote!(#pat => { #code })
}

#[test]
fn test_serialize_for_variant() {
    let input = parse_quote!(
        enum Transaction<'a> {
            Create { public_key: &'a PublicKey, name: &'a str },
            Transfer { from: &'a PublicKey, to: &'a PublicKey, amount: u64 }
        }
    );
    let s = Structure::new(&input);

    assert_eq!(
        serialize_for_variant(&s.variants()[0]),
        quote!(
            Transaction::Create {
                public_key: ref __binding_0,
                name: ref __binding_1,
            } => {
                let mut value = serializer.serialize_struct("Create", 2usize)?;
                value.serialize_field(
                    "public_key",
                    &__binding_0.serialize_field().map_err(S::Error::custom)?,
                )?;
                value.serialize_field(
                    "name",
                    &__binding_1.serialize_field().map_err(S::Error::custom)?,
                )?;
                value.end()
            }
        )
    )
}

fn impl_serialize(s: &Payload) -> quote::Tokens {
    let module = quote!(::exonum::encoding::serialize);
    let serialize_hands = s.variants().iter().map(serialize_for_variant);

    s.unbound_impl(
        quote!(#module::reexport::Serialize),
        quote!(
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: #module::reexport::Serializer,
            {
                use ::exonum::encoding::serialize::json::ExonumJson;
                use ::exonum::encoding::serialize::reexport::{SerError, SerializeStruct};

                match *self {
                    #(#serialize_hands)*
                }
            }
        ),
    )
}

fn deserialize_json_code(s: &Payload, variant: &VariantInfo) -> quote::Tokens {
    let ids_enum = &s.ids_enum_ident;
    let ident = &variant.ast().ident;
    let module = quote!(::exonum::encoding::serialize::json);

    let init = quote!(
        let header_size = ::exonum::encoding::MeasureHeader::header_size(&#ids_enum::#ident);
        let obj = stub.body.as_object().ok_or("Can't cast body as object.")?;
    );

    let code = execute_shifts(variant, |_, binding| {
        let field_type = strip_lifetimes(&binding.ast().ty);
        let field_name = binding
            .ast()
            .ident
            .as_ref()
            .expect("Unnamed fields not supported")
            .to_string();
        let field_name_err = format!("Cannot access field `{}`", field_name);

        quote!(
            let val = obj.get(#field_name).ok_or(#field_name_err)?;
            <#field_type as #module::ExonumJson>::deserialize_field(
                val, writer, __from, __from + __size,
            )?;
        )
    });

    quote!(
        #init
        let raw = stub.write(header_size as usize, |writer| {
            #code
            Ok(())
        })?;
        Ok(raw)
    )
}

#[test]
fn test_deserialize_json_code() {
    let input = parse_quote!(
        #[exonum(service_id = 10)]
        enum Transaction<'a> {
            Create { public_key: &'a PublicKey, name: &'a str },
            Transfer { from: &'a PublicKey, to: &'a PublicKey, amount: u64 }
        }
    );
    let s = Payload::try_from(Structure::new(&input)).unwrap();

    assert_eq!(
        deserialize_json_code(&s, &s.variants()[0]),
        quote!(
            let header_size = ::exonum::encoding::MeasureHeader::header_size(
                &TransactionIds::Create
            );
            let obj = stub.body.as_object().ok_or("Can't cast body as object.")?;

            let raw = stub.write(header_size as usize, |writer| {
                let __from = 0 as ::exonum::encoding::Offset;
                let __size = <&PublicKey as ::exonum::encoding::Field>::field_size();
                let val = obj.get("public_key").ok_or("Cannot access field `public_key`")?;
                <&PublicKey as ::exonum::encoding::serialize::json::ExonumJson>::deserialize_field(
                    val, writer, __from, __from + __size,
                )?;

                let __from = __from + __size;
                let __size = <&str as ::exonum::encoding::Field>::field_size();
                let val = obj.get("name").ok_or("Cannot access field `name`")?;
                <&str as ::exonum::encoding::serialize::json::ExonumJson>::deserialize_field(
                    val, writer, __from, __from + __size,
                )?;

                Ok(())
            })?;

            Ok(raw)
        )
    );
}

fn impl_json_deserialize(s: &Payload) -> quote::Tokens {
    let match_hands = s.variants().iter().enumerate().map(|(i, variant)| {
        let pat = s.id_pat(i);
        let code = deserialize_json_code(s, variant);
        quote!(#pat => { #code })
    });
    let service_id = &s.service_id;

    let module = quote!(::exonum::encoding::serialize::json);
    s.unbound_impl(
        quote!(#module::ExonumJsonDeserialize<::exonum::messages::RawMessage>),
        quote!(
            fn deserialize(value: &#module::reexport::Value)
                -> ::std::result::Result<::exonum::messages::RawMessage, Box<::std::error::Error>> {

                let stub = ::exonum::helpers::derive::DeStub::from_value(value).map_err(
                    Box::<::std::error::Error>::from,
                )?;

                if stub.service_id != #service_id {
                    return Err("service_id isn't equal real service_id.".into())
                }

                match stub.message_id {
                    #(#match_hands)*
                    _ => {
                        return Err("message_id not recognized".into());
                    },
                }
            }
        ),
    )
}
