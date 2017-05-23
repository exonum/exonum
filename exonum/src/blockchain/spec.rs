#[macro_export]
macro_rules! storage_value {
    ($(#[$attr:meta])* struct $name:ident {
        const SIZE = $body:expr;

        $($(#[$field_attr:meta])* field $field_name:ident : $field_type:ty [$from:expr => $to:expr])*
    }) => (
        #[derive(Clone, PartialEq)]
        $(#[$attr])*
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

            $(
            $(#[$field_attr])*
            pub fn $field_name(&self) -> $field_type {
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
                    let mut structure = serializer.serialize_struct(stringify!($name), counter!($($field_name)*))?;
                    $(structure.serialize_field(stringify!($field_name), &$crate::serialize::json::wrap(&self.$field_name()))?;)*
                    structure.end()               
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
            fn deserialize_owned(value: &::serde_json::Value) -> Result<Self, Box<::std::error::Error>> {
                let to = $body;
                let from = 0;
                use $crate::serialize::json::ExonumJsonDeserializeField;

                let mut buf = vec![0; $body];
                <Self as ExonumJsonDeserializeField>::deserialize(value, &mut buf, from, to)?; 
                Ok($name { raw: buf })
            }
        }

        //\TODO: Rewrite Deserialize and Serializa implementation
        impl<'de> $crate::serialize::json::reexport::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: $crate::serialize::json::reexport::Deserializer<'de>
            {
                use $crate::serialize::json::reexport::{Error, Deserialize, Value};
                let value = <Value as Deserialize>::deserialize(deserializer)?;
                <Self as $crate::serialize::json::ExonumJsonDeserialize>::deserialize_owned(&value)
                .map_err(|_| D::Error::custom("Can not deserialize value."))
            }
        }

        impl $crate::serialize::json::reexport::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: $crate::serialize::json::reexport::Serializer
                {
                    $crate::serialize::json::wrap(self).serialize(serializer)
                }
        }
    )
}
