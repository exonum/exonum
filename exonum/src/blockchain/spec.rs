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
            fn serialize(&self, mut buf: Vec<u8>) -> Vec<u8> {

                let old_len = buf.len(); 
                let new_len = old_len + self.raw.len(); 
                buf.resize(new_len, 0);
                {
                    let part = &mut buf[old_len..new_len]; 
                part.copy_from_slice(&self.raw); 
                }
                buf
            }

            fn deserialize(v: Vec<u8>) -> Self {
                $name {
                    raw: v
                }
            }

            fn len_hint(&self) -> usize {
                self.raw.len()
            }

            fn hash(&self) -> Hash {
                $name::hash(self)
            }
        }

        // TODO extract some fields like hash and from_raw into trait
        impl $name {
            pub fn new($($field_name: $field_type,)*) -> $name {
                use $crate::messages::{Field};
                let mut buf = vec![0; $body];
                $($field_name.write(&mut buf, $from, $to);)*
                $name { raw: buf }
            }

            pub fn from_raw(raw: Vec<u8>) -> $name {
                debug_assert_eq!(raw.len(), $body);
                $ name { raw: raw }
            }

            pub fn hash(&self) -> Hash {
                hash(self.raw.as_ref())
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
