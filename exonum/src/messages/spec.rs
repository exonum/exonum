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

        impl<'a> $crate::messages::Field<'a> for $name {
            fn read(buffer: &'a [u8], from: usize, to: usize) -> Self {
                let raw_message: $crate::messages::RawMessage =
                                    $crate::messages::Field::read(buffer, from, to);
                $crate::messages::FromRaw::from_raw(raw_message).unwrap()
            }

            fn write(&self, buffer: &mut Vec<u8>, from: usize, to: usize) {
                $crate::stream_struct::Field::write(&self.raw, buffer, from, to);
            }

            fn check(buffer: &'a [u8], from: usize, to: usize)
                -> Result<(), $crate::messages::Error>
            {

                let raw_message: $crate::messages::RawMessage =
                                    $crate::messages::Field::read(buffer, from, to);
                $(raw_message.check::<$field_type>($from, $to)?;)*
                Ok(None)
            }

            fn field_size() -> usize {
                $body
            }
        }

        impl $crate::messages::FromRaw for $name {
            fn from_raw(raw: $crate::messages::RawMessage)
                -> Result<$name, $crate::messages::Error> {
                $(raw.check::<$field_type>($from, $to)?;)*
                Ok($name { raw: raw })
            }
        }
        impl $name {
            #![cfg_attr(feature="cargo-clippy", allow(too_many_arguments))]
            pub fn new($($field_name: $field_type,)*
                       secret_key: &$crate::crypto::SecretKey) -> $name {
                use $crate::messages::{RawMessage, MessageWriter};
                let mut writer = MessageWriter::new($crate::messages::PROTOCOL_MAJOR_VERSION,
                                                    $crate::messages::TEST_NETWORK_ID,
                                                    $extension, $id, $body);
                $(writer.write($field_name, $from, $to);)*
                $name { raw: RawMessage::new(writer.sign(secret_key)) }
            }
            pub fn new_with_signature($($field_name: $field_type,)*
                       signature: &$crate::crypto::Signature) -> $name {
                use $crate::messages::{RawMessage, MessageWriter};
                let mut writer = MessageWriter::new($crate::messages::PROTOCOL_MAJOR_VERSION,
                                                    $crate::messages::TEST_NETWORK_ID,
                                                    $extension, $id, $body);
                $(writer.write($field_name, $from, $to);)*
                $name { raw: RawMessage::new(writer.append_signature(signature)) }

            }
            $(
            $(#[$field_attr])*
            pub fn $field_name(&self) -> $field_type {
                self.raw.read::<$field_type>($from, $to)
            })*
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter)
                -> Result<(), ::std::fmt::Error> {
                fmt.debug_struct(stringify!($name))
                 $(.field(stringify!($field_name), &self.$field_name()))*
                   .finish()
            }
        }

        impl $crate::stream_struct::serialize::json::ExonumJsonSerialize for $name {
                fn serialize<S>(&self, serializer: S) ->
                    Result<S::Ok, S::Error>
                where S: $crate::stream_struct::serialize::json::reexport::Serializer
                {
                    use $crate::stream_struct::serialize::json::reexport::SerializeStruct;
                    use $crate::stream_struct::serialize::json;

                    pub struct Body<'a>{_self: &'a $name};
                    impl<'a> $crate::stream_struct::serialize::json::reexport::Serialize for Body<'a> {
                        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                            where S: $crate::stream_struct::serialize::json::reexport::Serializer
                        {
                            let mut structure = serializer.serialize_struct(stringify!($name),
                                                            counter!($($field_name)*) )?;
                            $(structure.serialize_field(stringify!($field_name),
                                            &json::wrap(&self._self.$field_name()))?;)*

                            structure.end()
                        }
                    }

                    let mut structure = serializer.serialize_struct(stringify!($name), 4 )?;
                    structure.serialize_field("body", &Body{_self: &self})?;
                    structure.serialize_field("signature", &json::wrap(self.raw.signature()))?;
                    structure.serialize_field("message_id", &json::wrap(&self.raw.message_type()))?;
                    structure.serialize_field("service_id", &json::wrap(&self.raw.service_id()))?;
                    structure.serialize_field("network_id", &json::wrap(&self.raw.network_id()))?;
                    structure.serialize_field("protocol_version",&json::wrap(&self.raw.version()))?;
                    structure.end()
                }
        }

        impl $crate::stream_struct::serialize::json::ExonumJsonDeserializeField for $name {
            fn deserialize_field<B> (value: &$crate::stream_struct::serialize::json::reexport::Value,
                                        buffer: & mut B, from: usize, to: usize )
                -> Result<(), Box<::std::error::Error>>
            where B: $crate::stream_struct::serialize::json::WriteBufferWrapper
            {
                use $crate::stream_struct::serialize::json::ExonumJsonDeserialize;
                // deserialize full field
                let structure = <Self as ExonumJsonDeserialize>::deserialize(value)?;
                // then write it
                buffer.write(from, to, structure);
                Ok(())
            }
        }

        impl $crate::stream_struct::serialize::json::ExonumJsonDeserialize for $name {
            fn deserialize(value: &$crate::stream_struct::serialize::json::reexport::Value)
                -> Result<Self, Box<::std::error::Error>>
            {
                use $crate::stream_struct::serialize::json::ExonumJsonDeserializeField;
                use $crate::stream_struct::serialize::json::reexport::from_value;
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
                    <$field_type as ExonumJsonDeserializeField>::deserialize_field(val,
                                                                    &mut writer, $from, $to )?;
                )*

                Ok($name { raw: RawMessage::new(writer.append_signature(&signature)) })
            }
        }

        //\TODO: Rewrite Deserialize and Serializa implementation
        impl<'de> $crate::stream_struct::serialize::json::reexport::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: $crate::stream_struct::serialize::json::reexport::Deserializer<'de>
            {
                use $crate::stream_struct::serialize::json::reexport::{Error, Deserialize, Value};
                let value = <Value as Deserialize>::deserialize(deserializer)?;
                <Self as $crate::stream_struct::serialize::json::ExonumJsonDeserialize>::deserialize(&value)
                .map_err(|_| D::Error::custom("Can not deserialize value."))
            }
        }

        impl $crate::stream_struct::serialize::json::reexport::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: $crate::stream_struct::serialize::json::reexport::Serializer
                {
                    $crate::stream_struct::serialize::json::wrap(self).serialize(serializer)
                }
        }

    )
}
