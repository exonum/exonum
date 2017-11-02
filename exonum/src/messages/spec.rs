// Copyright 2017 The Exonum Team
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

/// `message!` implement structure that could be sent in exonum network.
///
/// Each message is a piece of data that is signed by creators key.
/// For now it's required to set service id as `const TYPE`, message id as `const ID`, and
/// message fixed part size as `const SIZE`.
///
/// - service id should be unique inside whole exonum.
/// - message id should be unique inside each service.
///
/// For each field, it's required to set exact position in message.
/// # Usage Example:
/// ```
/// #[macro_use] extern crate exonum;
/// # extern crate serde;
///
/// const MY_SERVICE_ID: u16 = 777;
/// const MY_NEW_MESSAGE_ID: u16 = 1;
///
/// message! {
///     struct SendTwoInteger {
///         const TYPE = MY_NEW_MESSAGE_ID;
///         const ID   = MY_SERVICE_ID;
///         const SIZE = 16;
///
///         field first: u64 [0 => 8]
///         field second: u64 [8 => 16]
///     }
/// }
///
/// # fn main() {
///     let (_, creators_key) = ::exonum::crypto::gen_keypair();
/// #    let structure = create_message(&creators_key);
/// #    println!("Debug structure = {:?}", structure);
/// # }
///
/// # fn create_message(creators_key: &::exonum::crypto::SecretKey) -> SendTwoInteger {
///     let first = 1u64;
///     let second = 2u64;
///     SendTwoInteger::new(first, second, creators_key)
/// # }
/// ```
///
/// For additionall reference about data layout see also
/// *[ `encoding` documentation](./encoding/index.html).*
///
/// `message!` internaly use `ident_count!`, be sure to add this macro to namespace.
#[macro_export]
macro_rules! message {
    (
    $(#[$attr:meta])*
    struct $name:ident {
        const TYPE = $extension:expr;
        const ID = $id:expr;
        const SIZE = $body:expr;

        $(
        $(#[$field_attr:meta])*
        field $field_name:ident : $field_type:ty [$from:expr => $to:expr]
        )*
    }) => (
        #[derive(Clone, PartialEq)]
        $(#[$attr])*
        pub struct $name {
            raw: $crate::messages::RawMessage
        }

        impl $crate::messages::Message for $name {
            fn raw(&self) -> &$crate::messages::RawMessage {
                &self.raw
            }
        }

        impl<'a> $crate::encoding::SegmentField<'a> for $name {

            fn item_size() -> $crate::encoding::Offset {
                1
            }

            fn count(&self) -> $crate::encoding::Offset {
                self.raw.len() as $crate::encoding::Offset
            }

            fn extend_buffer(&self, buffer: &mut Vec<u8>) {
                buffer.extend_from_slice(self.raw.as_ref().as_ref())
            }

            unsafe fn from_buffer(buffer: &'a [u8],
                                    from: $crate::encoding::Offset,
                                    count: $crate::encoding::Offset) -> Self {
                let raw_message: $crate::messages::RawMessage =
                                    $crate::encoding::SegmentField::from_buffer(buffer,
                                                                from,
                                                                count);
                $crate::messages::FromRaw::from_raw(raw_message).unwrap()
            }

            fn check_data(buffer: &'a [u8],
                    from: $crate::encoding::CheckedOffset,
                    count: $crate::encoding::CheckedOffset,
                    latest_segment: $crate::encoding::CheckedOffset)
              -> $crate::encoding::Result {
                let latest_segment_origin = <$crate::messages::RawMessage as
                                $crate::encoding::SegmentField>::check_data(buffer,
                                                                from,
                                                                count,
                                                                latest_segment)?;
                // TODO: remove this allocation,
                // by allowing creating message from borrowed data (ECR-156)
                let raw_message: $crate::messages::RawMessage =
                                    unsafe { $crate::encoding::SegmentField::from_buffer(buffer,
                                                                from.unchecked_offset(),
                                                                count.unchecked_offset())};
                let _: $name = $crate::messages::FromRaw::from_raw(raw_message)?;
                Ok(latest_segment_origin)
            }
        }

        impl $crate::messages::FromRaw for $name {
            fn from_raw(raw: $crate::messages::RawMessage)
                -> Result<$name, $crate::encoding::Error> {
                let min_message_size = $body as usize
                            + $crate::messages::HEADER_LENGTH as usize
                            + $crate::crypto::SIGNATURE_LENGTH as usize;
                if raw.len() < min_message_size {
                    return Err($crate::encoding::Error::UnexpectedlyShortPayload {
                        actual_size: raw.len() as $crate::encoding::Offset,
                        minimum_size: min_message_size as $crate::encoding::Offset,
                    });
                }

                let body_len = <Self>::check_fields(&raw)?;

                if body_len.unchecked_offset() as usize +
                    $crate::crypto::SIGNATURE_LENGTH as usize != raw.len()  {
                   return Err("Incorrect raw message length.".into())
                }

                Ok($name { raw: raw })
            }
        }
        impl $name {
            #[cfg_attr(feature="cargo-clippy", allow(too_many_arguments))]
            /// Creates messsage and sign it.
            #[allow(unused_mut)]
            pub fn new($($field_name: $field_type,)*
                       secret_key: &$crate::crypto::SecretKey) -> $name {
                use $crate::messages::{RawMessage, MessageWriter};
                check_bounds!($body, $($field_name : $field_type [$from => $to],)*);
                let mut writer = MessageWriter::new($crate::messages::PROTOCOL_MAJOR_VERSION,
                                                    $crate::messages::TEST_NETWORK_ID,
                                                    $extension, $id, $body);
                $(writer.write($field_name, $from, $to);)*
                $name { raw: RawMessage::new(writer.sign(secret_key)) }
            }

            /// Creates message and appends existing signature.
            #[cfg_attr(feature="cargo-clippy", allow(too_many_arguments))]
            #[allow(dead_code, unused_mut)]
            pub fn new_with_signature($($field_name: $field_type,)*
                       signature: &$crate::crypto::Signature) -> $name {
                use $crate::messages::{RawMessage, MessageWriter};
                check_bounds!($body, $($field_name : $field_type [$from => $to],)*);
                let mut writer = MessageWriter::new($crate::messages::PROTOCOL_MAJOR_VERSION,
                                                    $crate::messages::TEST_NETWORK_ID,
                                                    $extension, $id, $body);
                $(writer.write($field_name, $from, $to);)*
                $name { raw: RawMessage::new(writer.append_signature(signature)) }

            }

            #[allow(unused_variables)]
            fn check_fields(raw_message: &$crate::messages::RawMessage)
            -> $crate::encoding::Result {
                let latest_segment = (($body + $crate::messages::HEADER_LENGTH)
                                        as $crate::encoding::Offset).into();
                $(
                    let field_from: $crate::encoding::Offset = $from;
                    let field_to: $crate::encoding::Offset = $to;
                    let latest_segment = raw_message.check::<$field_type>(field_from.into(),
                                                    field_to.into(),
                                                    latest_segment)?;
                )*
                Ok(latest_segment)
            }

            /// Returns `message_id` useable for matching.
            #[allow(dead_code)]
            pub fn message_id() -> u16 {
                $id
            }

            /// Returns `service_id` useable for matching.
            #[allow(dead_code)]
            pub fn service_id() -> u16 {
                $extension
            }

            $(
            $(#[$field_attr])*
            pub fn $field_name(&self) -> $field_type {
                unsafe{ self.raw.read::<$field_type>($from, $to)}
            })*
        }

        impl $crate::storage::StorageValue for $name {
            fn hash(&self) -> $crate::crypto::Hash {
                $crate::messages::Message::hash(self)
            }

            fn into_bytes(self) -> Vec<u8> {
                self.raw.as_ref().as_ref().to_vec()
            }

            fn from_bytes(value: ::std::borrow::Cow<[u8]>) -> Self {
                $name {
                    raw: $crate::messages::RawMessage::new(
                        $crate::messages::MessageBuffer::from_vec(value.into_owned()))
                }
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter)
                -> Result<(), ::std::fmt::Error> {
                fmt.debug_struct(stringify!($name))
                 $(.field(stringify!($field_name), &self.$field_name()))*
                   .finish()
            }
        }

        impl $crate::encoding::serialize::json::ExonumJson for $name {
            fn deserialize_field<B> (value: &$crate::encoding::serialize::json::reexport::Value,
                                        buffer: & mut B,
                                        from: $crate::encoding::Offset,
                                        to: $crate::encoding::Offset )
                -> Result<(), Box<::std::error::Error>>
            where B: $crate::encoding::serialize::WriteBufferWrapper
            {
                use $crate::encoding::serialize::json::ExonumJsonDeserialize;
                // deserialize full field
                let structure = <Self as ExonumJsonDeserialize>::deserialize(value)?;
                // then write it
                buffer.write(from, to, structure);
                Ok(())
            }


            #[allow(unused_mut)]
            fn serialize_field(&self)
                -> Result<$crate::encoding::serialize::json::reexport::Value,
                            Box<::std::error::Error>>
            {
                use $crate::encoding::serialize::json::reexport::Value;
                use $crate::encoding::serialize::json::reexport::Map;
                let mut body = Map::new();
                $(
                    body.insert(stringify!($field_name).to_string(),
                        self.$field_name().serialize_field()?);
                )*
                let mut structure = Map::new();
                structure.insert("body".to_string(), Value::Object(body));
                structure.insert("signature".to_string(),
                                    self.raw.signature().serialize_field()?);
                structure.insert("message_id".to_string(),
                                    self.raw.message_type().serialize_field()?);
                structure.insert("service_id".to_string(),
                                    self.raw.service_id().serialize_field()?);
                structure.insert("network_id".to_string(),
                                    self.raw.network_id().serialize_field()?);
                structure.insert("protocol_version".to_string(),
                                    self.raw.version().serialize_field()?);
                Ok(Value::Object(structure))
            }
        }

        impl $crate::encoding::serialize::json::ExonumJsonDeserialize for $name {
            #[allow(unused_imports, unused_variables, unused_mut)]
            fn deserialize(value: &$crate::encoding::serialize::json::reexport::Value)
                -> Result<Self, Box<::std::error::Error>>
            {
                use $crate::encoding::serialize::json::ExonumJson;
                use $crate::encoding::serialize::json::reexport::from_value;
                use $crate::messages::{RawMessage, MessageWriter};

                // if we could deserialize values, try append signature
                let obj = value.as_object().ok_or("Can't cast json as object.")?;

                let body = obj.get("body").ok_or("Can't get body from json.")?;

                let signature = from_value(obj.get("signature")
                                    .ok_or("Can't get signature from json")?.clone())?;
                let message_type = from_value(obj.get("message_id")
                                    .ok_or("Can't get message_type from json")?.clone())?;
                let service_id = from_value(obj.get("service_id")
                                    .ok_or("Can't get service_id from json")?.clone())?;

                let network_id = from_value(obj.get("network_id")
                                    .ok_or("Can't get network_id from json")?.clone())?;
                let protocol_version = from_value(obj.get("protocol_version")
                                        .ok_or("Can't get protocol_version from json")?.clone())?;

                if service_id != $extension {
                    return Err("service_id didn't equal real service_id.".into())
                }

                if message_type != $id {
                    return Err("message_id didn't equal real message_id.".into())
                }

                let mut writer = MessageWriter::new(protocol_version, network_id,
                                                        service_id, message_type, $body);
                let obj = body.as_object().ok_or("Can't cast body as object.")?;
                $(
                    let val = obj.get(stringify!($field_name))
                                    .ok_or("Can't get object from json.")?;
                    <$field_type as ExonumJson>::deserialize_field(val,
                                                                    &mut writer, $from, $to )?;
                )*

                Ok($name { raw: RawMessage::new(writer.append_signature(&signature)) })
            }
        }

        // TODO: Rewrite Deserialize and Serialize implementation (ECR-156)
        impl<'de> $crate::encoding::serialize::reexport::Deserialize<'de> for $name {
            #[allow(unused_mut)]
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: $crate::encoding::serialize::reexport::Deserializer<'de>
            {
                use $crate::encoding::serialize::json::reexport::Value;
                use $crate::encoding::serialize::reexport::{DeError, Deserialize};
                let value = <Value as Deserialize>::deserialize(deserializer)?;
                <Self as $crate::encoding::serialize::json::ExonumJsonDeserialize>::deserialize(
                    &value).map_err(|e| D::Error::custom(
                            format!("Can't deserialize a value: {}", e.description())))
            }
        }

        impl $crate::encoding::serialize::reexport::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: $crate::encoding::serialize::reexport::Serializer
           {
               use $crate::encoding::serialize::reexport::SerError;
                use $crate::encoding::serialize::json::ExonumJson;
                self.serialize_field()
                    .map_err(|_| S::Error::custom(
                                concat!("Can not serialize structure: ", stringify!($name))))?
                    .serialize(serializer)
            }
        }

    );
}
