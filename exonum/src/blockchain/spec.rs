#[macro_export]
macro_rules! storage_value {
    (@count ) => {0};
    (@count $first:ident $($tail:ident)*) => {
        1usize + message!(@count $($tail)*)
    };
    ($name:ident {
        const SIZE = $body:expr;

        $($field_name:ident : $field_type:ty [$from:expr => $to:expr])*
    }) => (
        #[derive(Clone, PartialEq)]
        pub struct $name {
            raw: Vec<u8>
        }

        impl<'a> $crate::messages::Field<'a> for $name {
            fn read(buffer: &'a [u8], from: usize, to: usize) -> Self {
                let vec: Vec<u8> = $crate::messages::Field::read(buffer, from, to);
                $crate::storage::StorageValue::deserialize(vec)
            }

            fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
                $crate::messages::Field::write(&self.raw, buffer, from, to);
            }

            fn check(buffer: &'a [u8], from_st_val: usize, to_st_val: usize) -> Result<(), $crate::messages::Error> {

                <Vec<u8> as $crate::messages::Field>::check(buffer, from_st_val, to_st_val)?;
                let vec: Vec<u8> = $crate::messages::Field::read(buffer, from_st_val, to_st_val);
                $( <$field_type as $crate::messages::Field>::check(&vec, $from, $to)?;)*
                //$(raw_message.check::<$field_type>($from, $to)?;)*
                Ok(())
            }

            fn field_size() -> usize {
                1
            }
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

        impl $crate::serialize::json::ExonumJsonSerialize for $name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: ::serde::Serializer {
                    use ::serde::ser::SerializeStruct;
                    let mut strukt = serializer.serialize_struct(stringify!($name), storage_value!(@count $($field_name)*))?;
                    $(strukt.serialize_field(stringify!($field_name), &$crate::serialize::json::wrap(&self.$field_name()))?;)*
                    strukt.end()               
                }
        }

        impl $crate::serialize::json::ExonumJsonDeserialize for $name {
            fn deserialize_default(value: &::serde_json::Value) -> Option<Self> {
                let to = $body;
                let from = 0;
                use $crate::serialize::json::ExonumJsonDeserialize;

                let mut buf = vec![0; $body];
                

                if <Self as ExonumJsonDeserialize>::deserialize(value, &mut buf, from, to) {
                    Some($name { raw: buf })
                }
                else {
                    None
                }
            }
            fn deserialize<B: $crate::serialize::json::WriteBufferWrapper> (value: &::serde_json::Value, buffer: & mut B, from: usize, _to: usize ) -> bool {
                macro_rules! unwrap_option {
                    ($val:expr) => {if let Some(v) = $val {
                        v
                    } else {
                        return false;
                    }
                    }
                }
                let obj = unwrap_option!(value.as_object());
                $(
                let val = unwrap_option!(obj.get(stringify!($field_name)));

                if !<$field_type as $crate::serialize::json::ExonumJsonDeserialize>::deserialize(val, buffer, from + $from, from + $to )
                {
                    return false;
                }
                )*
                return true;
            }
        }

    )
}
