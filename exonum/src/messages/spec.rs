#[macro_export]
macro_rules! message {
    ($name:ident {
        const TYPE = $extension:expr;
        const ID = $id:expr;
        const SIZE = $body:expr;

        $($field_name:ident : $field_type:ty [$from:expr => $to:expr])*
    }) => (
        #[derive(Clone, PartialEq)]
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
                let raw_message: $crate::messages::RawMessage = $crate::messages::Field::read(buffer, from, to);
                $crate::messages::FromRaw::from_raw(raw_message).unwrap()
            }

            fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
                $crate::messages::Field::write(&self.raw, buffer, from, to);
            }

            fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<(), $crate::messages::Error> {

                let raw_message: $crate::messages::RawMessage = $crate::messages::Field::read(buffer, from, to);
                $(raw_message.check::<$field_type>($from, $to)?;)*
                Ok(())
            }

            fn field_size() -> usize {
                1
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
                let mut writer = MessageWriter::new($extension, $id, $body);
                $(writer.write($field_name, $from, $to);)*
                $name { raw: RawMessage::new(writer.sign(secret_key)) }
            }
            pub fn new_with_signature($($field_name: $field_type,)*
                       signature: &$crate::crypto::Signature) -> $name {
                use $crate::messages::{RawMessage, MessageWriter};
                let mut writer = MessageWriter::new($extension, $id, $body);
                $(writer.write($field_name, $from, $to);)*
                $name { raw: RawMessage::new(writer.append_signature(signature)) }

            }
            $(pub fn $field_name(&self) -> $field_type {
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

        impl $crate::serialize::json::ExonumJsonSerialize for $name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: $crate::serialize::json::reexport::Serializer {
                    use $crate::serialize::json::reexport::SerializeStruct;
                    let mut strukt = serializer.serialize_struct(stringify!($name), counter!($($field_name)*) + 1)?;
                    $(strukt.serialize_field(stringify!($field_name), &$crate::serialize::json::wrap(&self.$field_name()))?;)*
                    strukt.serialize_field("signature", &$crate::serialize::json::wrap(self.raw.signature()))?;
                    strukt.end()               
                }
        }

        impl $crate::serialize::json::ExonumJsonDeserializeField for $name {
            fn deserialize<B> (value: &$crate::serialize::json::reexport::Value, buffer: & mut B, from: usize, _to: usize ) -> Result<(), Box<::std::error::Error>>
            where B: $crate::serialize::json::WriteBufferWrapper
            {
                let obj = value.as_object().ok_or("Can't cast json as object.")?;
                $(
                    let val = obj.get(stringify!($field_name)).ok_or("Can't get object from json.")?;

                    <$field_type as $crate::serialize::json::ExonumJsonDeserializeField>::deserialize(val, buffer, from + $from, from + $to )?;

                )*
                Ok(())
            }
        }

        impl $crate::serialize::json::ExonumJsonDeserialize for $name {
            fn deserialize_owned(value: &$crate::serialize::json::reexport::Value) -> Result<Self, Box<::std::error::Error>> {
                let to = $body;
                let from = 0;
                use $crate::serialize::json::ExonumJsonDeserializeField;
                use $crate::messages::{RawMessage, MessageWriter};
                let mut writer = MessageWriter::new($extension, $id, $body);
                // if we could deserialize values, try append signature
                <Self as ExonumJsonDeserializeField>::deserialize(value, &mut writer, from, to)?;
                let obj = value.as_object().ok_or("Can't take json as object")?;
                let json_sign = obj.get("signature").ok_or("Can't get signature from json")?;
                
                let sign = $crate::serialize::json::reexport::from_value(json_sign.clone())?;
                Ok($name { raw: RawMessage::new(writer.append_signature(&sign)) })
            }
        }
    )
}
