#[macro_export]
macro_rules! storage_value {
    ($name:ident {
        const SIZE = $body:expr;

        $($field_name:ident : $field_type:ty [$from:expr => $to:expr])*
    }) => (
        #[derive(Clone, PartialEq)]
        pub struct $name {
            raw: Vec<u8>
        }

        impl $crate::storage::StorageValue for $name {
            fn serialize(self) -> Vec<u8> {
                self.raw
            }

            fn deserialize(v: Vec<u8>) -> Self {
                $name {
                    raw: v
                }
            }

            fn hash(&self) -> Hash {
                hash(self.raw.as_ref())
            }
        }

        impl $name {
            pub fn new($($field_name: $field_type,)*) -> $name {
                use $crate::messages::{Field};
                let mut buf = vec![0; $body];
                $($field_name.write(&mut buf, $from, $to);)*
                $name { raw: buf }
            }

            pub fn from_raw(raw: Vec<u8>) -> $name {
                // TODO: error instead of panic?
                assert_eq!(raw.len(), $body);
                Block { raw: raw }
            }

            $(pub fn $field_name(&self) -> $field_type {
                use $crate::messages::Field;
                Field::read(&self.raw, $from, $to)
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