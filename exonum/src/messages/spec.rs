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

        impl $crate::messages::FromRaw for $name {
            fn from_raw(raw: $crate::messages::RawMessage)
                -> Result<$name, $crate::messages::Error> {
                $(raw.check::<$field_type>($from, $to)?;)*
                Ok($name { raw: raw })
            }
        }

        impl $name {
            #![cfg_attr(feature="clippy", allow(too_many_arguments))]
            pub fn new($($field_name: $field_type,)*
                       secret_key: &$crate::crypto::SecretKey) -> $name {
                use $crate::messages::{RawMessage, MessageWriter};
                let mut writer = MessageWriter::new($extension, $id, $body);
                $(writer.write($field_name, $from, $to);)*
                $name { raw: RawMessage::new(writer.sign(secret_key)) }
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
