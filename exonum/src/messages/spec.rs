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
    )
}
