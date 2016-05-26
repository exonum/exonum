#[macro_export]
macro_rules! message {
    ($name:ident {
        const ID = $id:expr;
        const SIZE = $body:expr;

        $($field_name:ident : $field_type:ty [$from:expr => $to:expr])*
    }) => (
        #[derive(Clone)]
        pub struct $name {
            raw: $crate::messages::RawMessage
        }

        impl $crate::messages::Message for $name {
            const MESSAGE_TYPE : u16 = $id;
            const BODY_LENGTH : usize = $body;
            const PAYLOAD_LENGTH : usize =
                $body + $crate::crypto::SIGNATURE_LENGTH;
            const TOTAL_LENGTH : usize =
                $body + $crate::crypto::SIGNATURE_LENGTH
                      + $crate::messages::HEADER_SIZE;

            fn raw(&self) -> &$crate::messages::RawMessage {
                &self.raw
            }

            fn from_raw(raw: $crate::messages::RawMessage)
                -> Result<$name, $crate::messages::Error> {
                use $crate::messages::fields::Field;
                $(<$field_type>::check(raw.payload(), $from, $to)?;)*
                Ok($name { raw: raw })
            }
        }

        impl $name {
            pub fn new($($field_name: $field_type,)*
                       public_key: &$crate::crypto::PublicKey,
                       secret_key: &$crate::crypto::SecretKey) -> $name {
                use $crate::messages::{
                    RawMessage, MessageBuffer, Message, Field
                };
                let mut raw = MessageBuffer::new(Self::MESSAGE_TYPE,
                                              Self::PAYLOAD_LENGTH,
                                              public_key);
                {
                    let mut buffer = raw.as_mut();
                    $(
                    let from = $from + $crate::messages::HEADER_SIZE;
                    let to = $to + $crate::messages::HEADER_SIZE;
                    $field_name.write(&mut buffer, from, to);
                    )*
                }
                raw.sign(secret_key);
                $name { raw: RawMessage::new(raw) }
            }
            $(pub fn $field_name(&self) -> $field_type {
                use $crate::messages::fields::Field;
                <$field_type>::read(self.raw.payload(), $from, $to)
            })*
        }
    )
}
