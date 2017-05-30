/// `storage_value!` implement structure that could be saved in blockchain.
///
/// Storage value unlike message, could be mapped on buffers without any checks.
/// For now it's required to set `storage_value` fixed part size as `const SIZE`.
///
/// For each field, it's required to set exact position in `storage_value`.
/// # Usage Example:
/// ```
/// #[macro_use] extern crate exonum;
/// # extern crate serde;
/// # extern crate serde_json;
///
/// storage_value! {
///     struct SaveTwoInteger {
///         const SIZE = 16;
///         
///         field first: u64 [0 => 8]
///         field second: u64 [8 => 16]
///     }
/// }
/// # fn main() {
///     let first = 1u64;
///     let second = 2u64;
///     let s = SaveTwoInteger::new(first, second);
///     println!("Debug structure = {:?}", s);
/// # }
/// ```
///
/// For additionall reference about data layout see also 
/// *[ `stream_struct` documentation](./stream_struct/index.html).*
/// 
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
            unsafe fn read(buffer: &'a [u8],
                            from: $crate::stream_struct::Offset,
                            to: $crate::stream_struct::Offset) -> Self {
                let vec: Vec<u8> = $crate::stream_struct::Field::read(buffer, from, to);
                $crate::storage::StorageValue::deserialize(vec)
            }

            fn write(&self,
                            buffer: &mut Vec<u8>,
                            from: $crate::stream_struct::Offset,
                            to: $crate::stream_struct::Offset) {
                $crate::stream_struct::Field::write(&self.raw, buffer, from, to);
            }

            fn check(buffer: &'a [u8],
                        from_st_val: $crate::stream_struct::CheckedOffset,
                        to_st_val: $crate::stream_struct::CheckedOffset)
                -> $crate::stream_struct::Result
            {
                let ret = <Vec<u8> as $crate::stream_struct::Field>::check(buffer, from_st_val, to_st_val)?;
                let vec: Vec<u8> = unsafe{ $crate::stream_struct::Field::read(buffer, 
                                                                        from_st_val.unchecked_offset(),
                                                                        to_st_val.unchecked_offset())};
                let mut last_data = ($body as $crate::stream_struct::Offset).into();
                $( 
                    println!("check_field {} = {:?} {:?}", stringify!($field_name), $from, $to);
                    <$field_type as $crate::stream_struct::Field>::check(&vec,
                                                                        $from.into(),
                                                                        $to.into())?
                        .map_or(Ok(()), |mut e| e.check_segment(&mut last_data))?;
                )*
                Ok(ret)
            }

            fn field_size() -> $crate::stream_struct::Offset {
                $body as $crate::stream_struct::Offset
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
                unsafe {
                    Field::read(&self.raw, $from, $to)
                }
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
                                        buffer: & mut B,
                                        from: $crate::stream_struct::Offset,
                                        _to: $crate::stream_struct::Offset )
                -> Result<(), Box<::std::error::Error>>
                where B: $crate::stream_struct::serialize::WriteBufferWrapper
            {
                use $crate::stream_struct::serialize::json::ExonumJsonDeserializeField;
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
                let to = $body as $crate::stream_struct::Offset;
                let from = 0;
                use $crate::stream_struct::serialize::json::ExonumJsonDeserializeField;

                let mut buf = vec![0; $body];
                <Self as ExonumJsonDeserializeField>::deserialize_field(value, &mut buf, from, to)?;
                Ok($name { raw: buf })
            }
        }

        //\TODO: Rewrite Deserialize and Serializa implementation
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
