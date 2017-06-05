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
///     let (p, creators_key) = ::exonum::crypto::gen_keypair();
/// #    let stucture = create_message(&creators_key);
/// #    println!("Debug structure = {:?}", stucture);
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
/// *[ `stream_struct` documentation](./stream_struct/index.html).*
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

        impl<'a> $crate::stream_struct::Field<'a> for $name {
            unsafe fn read(buffer: &'a [u8],
                           from: $crate::stream_struct::Offset,
                           to: $crate::stream_struct::Offset) -> Self {
                let raw_message: $crate::messages::RawMessage = $crate::stream_struct::Field::read(buffer, from, to);
                $crate::messages::FromRaw::from_raw(raw_message).unwrap()
            }

            fn write(&self,
                            buffer: &mut Vec<u8>,
                            from: $crate::stream_struct::Offset,
                            to: $crate::stream_struct::Offset) {
                $crate::stream_struct::Field::write(&self.raw, buffer, from, to);
            }

            fn check(buffer: &'a [u8],
                     from: $crate::stream_struct::CheckedOffset,
                     to: $crate::stream_struct::CheckedOffset) -> $crate::stream_struct::Result {
                let check = <$crate::messages::RawMessage as
                                $crate::stream_struct::Field>::check(buffer,
                                                                from,
                                                                to)?;
                let raw_message: $crate::messages::RawMessage = 
                                    unsafe { $crate::stream_struct::Field::read(buffer,
                                                                from.unchecked_offset(),
                                                                to.unchecked_offset())};
                <Self>::check_fields(&raw_message)?;
                Ok(check)
            }

            fn field_size() -> $crate::stream_struct::Offset {
                // We write message as regular buffer,
                // so real `field_size` is 8.
                8 as $crate::stream_struct::Offset
            }
        }

        impl $crate::messages::FromRaw for $name {
            fn from_raw(raw: $crate::messages::RawMessage)
                -> Result<$name, $crate::stream_struct::Error> {
                <Self>::check_fields(&raw)?;
                Ok($name { raw: raw })
            }
        }
        impl $name {
            #[cfg_attr(feature="cargo-clippy", allow(too_many_arguments))]
            /// Create messsage `$name` and sign it.
            pub fn new($($field_name: $field_type,)*
                       secret_key: &$crate::crypto::SecretKey) -> $name {
                use $crate::messages::{RawMessage, MessageWriter};
                let mut writer = MessageWriter::new($crate::messages::PROTOCOL_MAJOR_VERSION,
                                                    $crate::messages::TEST_NETWORK_ID,
                                                    $extension, $id, $body);
                $(writer.write($field_name, $from, $to);)*
                $name { raw: RawMessage::new(writer.sign(secret_key)) }
            }

            /// Create message `$name` and append existing signature.
            #[cfg_attr(feature="cargo-clippy", allow(too_many_arguments))]
            pub fn new_with_signature($($field_name: $field_type,)*
                       signature: &$crate::crypto::Signature) -> $name {
                use $crate::messages::{RawMessage, MessageWriter};
                let mut writer = MessageWriter::new($crate::messages::PROTOCOL_MAJOR_VERSION,
                                                    $crate::messages::TEST_NETWORK_ID,
                                                    $extension, $id, $body);
                $(writer.write($field_name, $from, $to);)*
                $name { raw: RawMessage::new(writer.append_signature(signature)) }

            }

            fn check_fields(raw_message: &$crate::messages::RawMessage) -> $crate::stream_struct::Result {
                let mut last_data = (($body + $crate::messages::HEADER_SIZE)
                                        as $crate::stream_struct::Offset).into();
                $(
                    let field_from: $crate::stream_struct::Offset = $from;
                    let field_to: $crate::stream_struct::Offset = $to;
                    raw_message.check::<$field_type>(field_from.into(),field_to.into())?
                        .map_or(Ok(()), |mut e| e.check_segment(&mut last_data))?;
                )*
                Ok(None)
            }

            $(
            $(#[$field_attr])*
            pub fn $field_name(&self) -> $field_type {
                unsafe{ self.raw.read::<$field_type>($from, $to)}
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
                where S: $crate::stream_struct::serialize::reexport::Serializer
                {
                    use $crate::stream_struct::serialize::reexport::SerializeStruct;
                    use $crate::stream_struct::serialize::json;

                    pub struct Body<'a>{_self: &'a $name};
                    impl<'a> $crate::stream_struct::serialize::reexport::Serialize for Body<'a> {
                        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                            where S: $crate::stream_struct::serialize::reexport::Serializer
                        {
                            let mut structure = serializer.serialize_struct(stringify!($name),
                                                            idents_count!($($field_name)*) )?;
                            $(structure.serialize_field(stringify!($field_name),
                                            &json::wrap(&self._self.$field_name()))?;)*

                            structure.end()
                        }
                    }

                    let mut structure = serializer.serialize_struct(stringify!($name), 6 )?;
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
                                        buffer: & mut B,
                                        from: $crate::stream_struct::Offset,
                                        to: $crate::stream_struct::Offset )
                -> Result<(), Box<::std::error::Error>>
            where B: $crate::stream_struct::serialize::WriteBufferWrapper
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

        //\TODO: Rewrite Deserialize and Serialize implementation
        impl<'de> $crate::stream_struct::serialize::reexport::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: $crate::stream_struct::serialize::reexport::Deserializer<'de>
            {
                use $crate::stream_struct::serialize::json::reexport::Value;
                use $crate::stream_struct::serialize::reexport::{Error, Deserialize};
                let value = <Value as Deserialize>::deserialize(deserializer)?;
                <Self as $crate::stream_struct::serialize::json::ExonumJsonDeserialize>::deserialize(&value)
                .map_err(|_| D::Error::custom("Can not deserialize value."))
            }
        }

        impl $crate::stream_struct::serialize::reexport::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: $crate::stream_struct::serialize::reexport::Serializer
                {
                    $crate::stream_struct::serialize::json::wrap(self).serialize(serializer)
                }
        }

    )
}
