#[macro_export]
macro_rules! storage_value {
    (
    $(#[$attr:meta])*
    struct $name:ident {
        const SIZE = $body:expr;

        $(
        $(#[$field_attr:meta])*
        field $field_name:ident : $field_type:ty [$from:expr => $to:expr]
        )*
    }) => (
        #[derive(Clone, PartialEq)]
        $(#[$attr])*
        pub struct $name {
            raw: Vec<u8>
        }

        impl<'a> $crate::stream_struct::Field<'a> for $name {
            fn read(buffer: &'a [u8], from: usize, to: usize) -> Self {
                let vec: Vec<u8> = $crate::stream_struct::Field::read(buffer, from, to);
                $crate::storage::StorageValue::deserialize(vec)
            }

            fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
                $crate::stream_struct::Field::write(&self.raw, buffer, from, to);
            }

            fn check(buffer: &'a [u8], from_st_val: usize, to_st_val: usize)
                -> Result<(), $crate::stream_struct::Error>
            {
                <Vec<u8> as $crate::stream_struct::Field>::check(buffer, from_st_val, to_st_val)?;
                let vec: Vec<u8> = $crate::stream_struct::Field::read(buffer, from_st_val, to_st_val);
                $( <$field_type as $crate::stream_struct::Field>::check(&vec, $from, $to)?;)*
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

            fn hash(&self) -> $crate::crypto::Hash {
                $name::hash(self)
            }
        }

        // TODO extract some fields like hash and from_raw into trait
        impl $name {
            pub fn new($($field_name: $field_type,)*) -> $name {
                use $crate::stream_struct::{Field};
                let mut buf = vec![0; $body];
                $($field_name.write(&mut buf, $from, $to);)*
                $name { raw: buf }
            }

            #[allow(dead_code)]
            pub fn from_raw(raw: Vec<u8>) -> $name {
                debug_assert_eq!(raw.len(), $body);
                $ name { raw: raw }
            }

            pub fn hash(&self) -> $crate::crypto::Hash {
                $crate::crypto::hash(self.raw.as_ref())
            }

            $(
            $(#[$field_attr])*
            pub fn $field_name(&self) -> $field_type {
                use $crate::stream_struct::Field;
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

        impl $crate::stream_struct::serialize::json::ExonumJsonSerialize for $name {
                fn serialize<S>(&self, serializer: S)
                    -> Result<S::Ok, S::Error> where S: ::serde::Serializer
                {
                    use ::serde::ser::SerializeStruct;
                    let mut structure = serializer.serialize_struct(stringify!($name),
                                                    counter!($($field_name)*))?;
                    $(
                        structure.serialize_field(stringify!($field_name),
                            &$crate::stream_struct::serialize::json::wrap(&self.$field_name()))?;
                    )*
                    structure.end()
                }
        }

        impl $crate::stream_struct::serialize::json::ExonumJsonDeserializeField for $name {
            fn deserialize_field<B> (value: &$crate::stream_struct::serialize::json::reexport::Value,
                                        buffer: & mut B, from: usize, _to: usize )
                -> Result<(), Box<::std::error::Error>>
                where B: $crate::stream_struct::serialize::json::WriteBufferWrapper
            {
                use $crate::stream_struct::stream_struct::serialize::json::ExonumJsonDeserializeField;
                let obj = value.as_object().ok_or("Can't cast json as object.")?;
                $(
                let val = obj.get(stringify!($field_name)).ok_or("Can't get object from json.")?;

                <$field_type as ExonumJsonDeserializeField>::deserialize_field(val, buffer,
                                                                from + $from, from + $to )?;

                )*
                Ok(())
            }
        }
        impl $crate::stream_struct::serialize::json::ExonumJsonDeserialize for $name {
            fn deserialize(value: &::serde_json::Value) -> Result<Self, Box<::std::error::Error>> {
                let to = $body;
                let from = 0;
                use $crate::stream_struct::serialize::json::ExonumJsonDeserializeField;

                let mut buf = vec![0; $body];
                <Self as ExonumJsonDeserializeField>::deserialize_field(value, &mut buf, from, to)?;
                Ok($name { raw: buf })
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
