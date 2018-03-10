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

//! `Transaction` derivation.

use quote;
use syn::Path;
use synstructure::Structure;

use structure::extract_payload;

pub fn message_derive(s: Structure) -> quote::Tokens {
    let payload = extract_payload(&s.ast().attrs).unwrap();

    let wrapper = define_message_wrapper(&s, &payload);
    let debug = impl_debug(&s);
    let as_ref = impl_as_ref(&s);
    let from_hex = impl_from_hex(&s);
    let message = impl_message(&s, &payload);
    let json_deserialize = impl_json_deserialize(&s, &payload);
    let json = impl_json(&s);
    let serialize = impl_serialize(&s);
    let deserialize = impl_deserialize(&s);

    quote!(
        #wrapper
        #debug
        #as_ref
        #from_hex
        #message
        #json_deserialize
        #json
        #serialize
        #deserialize
    )
}

fn define_message_wrapper(s: &Structure, payload: &Path) -> quote::Tokens {
    let message = &s.ast().ident;

    quote!(
        impl #message {
            /// Returns the payload of the message.
            pub fn payload(&self) -> #payload {
                unsafe { <#payload as ::exonum::messages::Read>::unchecked_read(&self.0) }
            }
        }
    )
}

#[test]
fn test_define_message_wrapper() {
    let input = parse_quote!(
        #[exonum(payload = "Transaction")]
        pub struct Message(::exonum::messages::RawMessage);
    );
    let s = Structure::new(&input);
    let payload = parse_quote!(Transaction);

    assert_eq!(
        define_message_wrapper(&s, &payload),
        quote!(
            impl Message {
                /// Returns the payload of the message.
                pub fn payload(&self) -> Transaction {
                    unsafe { <Transaction as ::exonum::messages::Read>::unchecked_read(&self.0) }
                }
            }
        )
    );
}

fn impl_message(s: &Structure, payload: &Path) -> quote::Tokens {
    let message = &s.ast().ident;

    let check = s.unbound_impl(
        quote!(::exonum::messages::Check),
        quote!(
            fn check(raw: &::exonum::messages::RawMessage) ->
                ::std::result::Result<(), ::exonum::encoding::Error> {
                <#payload as ::exonum::messages::Check>::check(raw)
            }
        ),
    );

    let read = quote!(
        impl<'a> ::exonum::messages::Read<'a> for #message {
            unsafe fn unchecked_read(raw: &'a ::exonum::messages::RawMessage) -> Self {
                #message(raw.clone())
            }
        }
    );

    let message_impl = s.unbound_impl(
        quote!(::exonum::messages::Message),
        quote!(
            fn from_raw(raw: ::exonum::messages::RawMessage) ->
                ::std::result::Result<Self, ::exonum::encoding::Error> {

                <Self as ::exonum::messages::Check>::check(&raw)?;
                Ok(#message(raw))
            }

            fn raw(&self) -> &::exonum::messages::RawMessage {
                &self.0
            }
        ),
    );

    quote!(#check #read #message_impl)
}

fn impl_json_deserialize(s: &Structure, payload: &Path) -> quote::Tokens {
    let message = &s.ast().ident;
    let module = quote!(::exonum::encoding::serialize::json);

    s.unbound_impl(
        quote!(#module::ExonumJsonDeserialize),
        quote!(
            fn deserialize(value: &#module::reexport::Value)
                -> ::std::result::Result<Self, Box<::std::error::Error>> {
                <#payload as #module::ExonumJsonDeserialize<::exonum::messages::RawMessage>>
                    ::deserialize(value).map(#message)
            }
        ),
    )
}

fn impl_json(s: &Structure) -> quote::Tokens {
    let module = quote!(::exonum::encoding::serialize::json);

    s.unbound_impl(
        quote!(#module::ExonumJson),
        quote!(
            fn deserialize_field<B> (
                value: &#module::reexport::Value,
                buffer: &mut B,
                from: ::exonum::encoding::Offset,
                to: ::exonum::encoding::Offset,
            ) -> ::std::result::Result<(), Box<::std::error::Error>>
            where
                B: ::exonum::encoding::serialize::WriteBufferWrapper
            {
                let structure = <Self as #module::ExonumJsonDeserialize>::deserialize(value)?;
                buffer.write(from, to, structure);
                Ok(())
            }

            fn serialize_field(&self) -> ::std::result::Result<#module::reexport::Value,
                Box<::std::error::Error + Send + Sync>>
            {
                use ::exonum::messages::Message;

                let stub = ::exonum::helpers::derive::MessageStub::new(self.raw(), self.payload());
                Ok(#module::reexport::to_value(stub)?)
            }
        ),
    )
}

fn impl_serialize(s: &Structure) -> quote::Tokens {
    let module = quote!(::exonum::encoding::serialize);

    s.unbound_impl(
        quote!(#module::reexport::Serialize),
        quote!(
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: #module::reexport::Serializer,
            {
                use #module::reexport::SerError;
                use #module::json::ExonumJson;

                self.serialize_field()
                    .map_err(S::Error::custom)?
                    .serialize(serializer)
            }
        ),
    )
}

fn impl_deserialize(s: &Structure) -> quote::Tokens {
    let module = quote!(::exonum::encoding::serialize);
    let ident = &s.ast().ident;

    quote!(
        impl<'de> #module::reexport::Deserialize<'de> for #ident {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: #module::reexport::Deserializer<'de>,
            {
                use #module::json::reexport::Value;
                use #module::reexport::{DeError, Deserialize};

                let value = <Value as Deserialize>::deserialize(deserializer)?;
                <Self as #module::json::ExonumJsonDeserialize>::deserialize(&value)
                    .map_err(D::Error::custom)
            }
        }
    )
}

fn impl_from_hex(s: &Structure) -> quote::Tokens {
    let module = quote!(::exonum::encoding::serialize);

    s.unbound_impl(
        quote!(#module::FromHex),
        quote!(
            type Error = ::exonum::encoding::Error;

            fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
                ::exonum::helpers::derive::message_from_hex(hex)
            }
        ),
    )
}

fn impl_debug(s: &Structure) -> quote::Tokens {
    let ident_str = s.ast().ident.to_string();

    s.unbound_impl(
        quote!(::std::fmt::Debug),
        quote!(
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter)
                -> ::std::result::Result<(), ::std::fmt::Error>
            {
                fmt.debug_struct(#ident_str)
                    .field("payload", &self.payload())
                    .finish()
            }
        ),
    )
}

fn impl_as_ref(s: &Structure) -> quote::Tokens {
    s.unbound_impl(
        quote!(::std::convert::AsRef<::exonum::messages::RawMessage>),
        quote!(
            fn as_ref(&self) -> &::exonum::messages::RawMessage {
                ::exonum::messages::Message::raw(self)
            }
        ),
    )
}
