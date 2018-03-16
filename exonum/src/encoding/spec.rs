// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// `encoding_struct!` macro implements a structure that can be saved in the Exonum blockchain.
///
/// The macro creates getter methods for all fields with the same names as fields.
/// In addition, the macro declares a `new` constructor, which accepts all fields
/// in the order of their declaration in the macro.
/// The macro also implements [`Field`], [`ExonumJson`] and [`StorageValue`] traits
/// for the declared datatype.
///
/// Unlike types created with [`transactions!`], the datatype is mapped to a byte buffer
/// without any checks; it is assumed that the relevant checks have been performed
/// when persisting the structure to the blockchain storage.
///
/// For additional reference about data layout see the
/// documentation of the [`encoding` module](./encoding/index.html).
///
/// **NB.** `encoding_struct!` uses other macros in the `exonum` crate internally.
/// Be sure to add them to the global scope.
///
/// [`Field`]: ./encoding/trait.Field.html
/// [`ExonumJson`]: ./encoding/serialize/json/trait.ExonumJson.html
/// [`StorageValue`]: ./storage/trait.StorageValue.html
/// [`transactions!`]: macro.transactions.html
///
/// # Examples
///
/// ```
/// #[macro_use] extern crate exonum;
///
/// encoding_struct! {
///     struct SaveTwoIntegers {
///         first: u64,
///         second: u64,
///     }
/// }
///
/// # fn main() {
/// let s = SaveTwoIntegers::new(1, 2);
/// println!("Two integers: {:?}", s);
/// # }
/// ```
#[macro_export]
macro_rules! encoding_struct {
    (
    $(#[$attr:meta])*
    struct $name:ident {
        $(
        $(#[$field_attr:meta])*
        $field_name:ident : $field_type:ty
        ),*
        $(,)*
    }) => (
        #[derive(Clone, PartialEq)]
        $(#[$attr])*
        pub struct $name {
            raw: Vec<u8>
        }

        // Re-implement `Field` for `encoding_struct!`
        // to write fields in place of another structure
        #[allow(unsafe_code)]
        impl<'a> $crate::encoding::Field<'a> for $name {
            unsafe fn read(buffer: &'a [u8],
                            from: $crate::encoding::Offset,
                            to: $crate::encoding::Offset) -> Self {
                let vec: Vec<u8> = $crate::encoding::Field::read(buffer, from, to);
                $crate::storage::StorageValue::from_bytes(::std::borrow::Cow::Owned(vec))
            }

            fn write(&self,
                            buffer: &mut Vec<u8>,
                            from: $crate::encoding::Offset,
                            to: $crate::encoding::Offset) {
                $crate::encoding::Field::write(&self.raw, buffer, from, to);
            }

            #[allow(unused_variables)]
            #[allow(unused_comparisons)]
            fn check(buffer: &'a [u8],
                        from_st_val: $crate::encoding::CheckedOffset,
                        to_st_val: $crate::encoding::CheckedOffset,
                        latest_segment: $crate::encoding::CheckedOffset)
                -> $crate::encoding::Result
            {
                let latest_segment_origin = <&[u8] as $crate::encoding::Field>::check(
                    buffer, from_st_val, to_st_val, latest_segment)?;
                let vec: &[u8] = unsafe{ $crate::encoding::Field::read(
                    buffer,
                    from_st_val.unchecked_offset(),
                    to_st_val.unchecked_offset())};
                let latest_segment: $crate::encoding::CheckedOffset =
                    $name::__ex_header_size().into();

                if vec.len() < $name::__ex_header_size() as usize {
                    return Err($crate::encoding::Error::UnexpectedlyShortPayload{
                        actual_size: vec.len() as $crate::encoding::Offset,
                        minimum_size: $name::__ex_header_size() as $crate::encoding::Offset
                    })
                }

                __ex_for_each_field!(
                    __ex_struct_check_field, (latest_segment, vec),
                    $( ($(#[$field_attr])*, $field_name, $field_type) )*
                );
                Ok(latest_segment_origin)
            }

            fn field_size() -> $crate::encoding::Offset {
                // We write `encoding_struct` as regular buffer,
                // so real `field_size` is 8.
                // TODO: maybe we should write it as sub structure in place?
                // We could get benefit from it: we limit indirection
                // in deserializing sub fields, by only one calculation (ECR-156).

                // $body as $crate::encoding::Offset

                8 as $crate::encoding::Offset
            }
        }

        impl $crate::crypto::CryptoHash for $name {
            fn hash(&self) -> $crate::crypto::Hash {
                $crate::crypto::hash(self.raw.as_ref())
            }
        }

        impl $crate::storage::StorageValue for $name {
            fn into_bytes(self) -> Vec<u8> {
                self.raw
            }

            fn from_bytes(v: ::std::borrow::Cow<[u8]>) -> Self {
                $name {
                    raw: v.into_owned()
                }
            }
        }

        // TODO extract some fields like hash and from_raw into trait (ECR-156)
        impl $name {
            #[cfg_attr(feature="cargo-clippy", allow(too_many_arguments))]
            #[allow(unused_imports, unused_mut)]

            /// Creates a new instance with given parameters.
            pub fn new($($field_name: $field_type,)*) -> $name {
                let mut buf = vec![0; $name::__ex_header_size() as usize];
                __ex_for_each_field!(
                    __ex_struct_write_field, (buf),
                    $( ($(#[$field_attr])*, $field_name, $field_type) )*
                );
                $name { raw: buf }
            }

            __ex_for_each_field!(
                __ex_struct_mk_field, (),
                $( ($(#[$field_attr])*, $field_name, $field_type) )*
            );

            fn __ex_header_size() -> $crate::encoding::Offset {
                __ex_header_size!($($field_type),*)
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter)
                -> Result<(), ::std::fmt::Error> {
                fmt.debug_struct(stringify!($name))
                 $(.field(stringify!($field_name), &self.$field_name()))*
                   .finish()
            }
        }

        impl $crate::encoding::serialize::json::ExonumJson for $name {
            #[allow(unused_variables)]
            fn deserialize_field<B> (value: &$crate::encoding::serialize::json::reexport::Value,
                                        buffer: & mut B,
                                        from: $crate::encoding::Offset,
                                        to: $crate::encoding::Offset )
                -> Result<(), Box<::std::error::Error>>
                where B: $crate::encoding::serialize::WriteBufferWrapper
            {
                use $crate::encoding::serialize::json::ExonumJsonDeserialize;
                // deserialize full field
                let structure = <Self as ExonumJsonDeserialize>::deserialize(value)?;
                // then write it
                buffer.write(from, to, structure);

                Ok(())
            }


            #[allow(unused_mut)]
            fn serialize_field(&self)
                -> Result<$crate::encoding::serialize::json::reexport::Value,
                          Box<::std::error::Error + Send + Sync>>
            {
                use $crate::encoding::serialize::json::reexport::Value;
                let mut map = $crate::encoding::serialize::json::reexport::Map::new();
                $(
                    map.insert(stringify!($field_name).to_string(),
                        self.$field_name().serialize_field()?);
                )*
                Ok(Value::Object(map))
            }
        }
        impl $crate::encoding::serialize::json::ExonumJsonDeserialize for $name {
            #[allow(unused_imports, unused_mut)]
            fn deserialize(value: &$crate::encoding::serialize::json::reexport::Value)
                -> Result<Self, Box<::std::error::Error>> {
                use $crate::encoding::serialize::json::ExonumJson as ExonumJson;
                let mut buf = vec![0; $name::__ex_header_size() as usize];
                let _obj = value.as_object().ok_or("Can't cast json as object.")?;
                __ex_for_each_field!(
                    __ex_deserialize_field, (_obj, buf),
                    $( ($(#[$field_attr])*, $field_name, $field_type) )*
                );
                Ok($name { raw: buf })
            }
        }

        // TODO: Rewrite Deserialize and Serialize implementation (ECR-156)
        impl<'de> $crate::encoding::serialize::reexport::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: $crate::encoding::serialize::reexport::Deserializer<'de>
            {
                use $crate::encoding::serialize::json::reexport::Value;
                use $crate::encoding::serialize::reexport::{DeError, Deserialize};
                let value = <Value as Deserialize>::deserialize(deserializer)?;
                <Self as $crate::encoding::serialize::json::ExonumJsonDeserialize>::deserialize(
                    &value).map_err(|_| D::Error::custom("Can not deserialize value."))
            }
        }

        impl $crate::encoding::serialize::reexport::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: $crate::encoding::serialize::reexport::Serializer
            {
                use $crate::encoding::serialize::reexport::SerError;
                use $crate::encoding::serialize::json::ExonumJson;
                self.serialize_field()
                    .map_err(|_| S::Error::custom(
                                concat!("Can not serialize structure: ", stringify!($name))))?
                    .serialize(serializer)
            }
        }
    )
}

/// This macro checks bounds of fields for structs with custom layout.
#[macro_export]
macro_rules! check_bounds {
    (@deep $size:expr, $prev_to:expr,
     $field_name:ident : $field_type:ty [$field_from:expr => $field_to:expr],
     $($next_name:ident : $next_type:ty [$next_from:expr => $next_to:expr],)+
     ) => {
        debug_assert_eq!($prev_to, $field_from, "fields should be adjacent");
        debug_assert_eq!($field_to - $field_from, <$field_type as Field>::field_size(),
            "wrong size of field");
        check_bounds!(@deep $size, $field_to,
            $($next_name : $next_type [$next_from => $next_to],)+);
    };
    (@deep $size:expr, $prev_to:expr,
     $last_name:ident : $last_type:ty [$last_from:expr => $last_to:expr],
     ) => {
        debug_assert_eq!($prev_to, $last_from, "fields should be adjacent");
        debug_assert_eq!($last_to, $size, "last field should matches the size of struct");
        debug_assert_eq!($last_to - $last_from, <$last_type as Field>::field_size(),
            "wrong size of field");
    };
    ($size:expr,
     $first_name:ident : $first_type:ty [$first_from:expr => $first_to:expr],
     ) => {{
        use $crate::encoding::Field;
        debug_assert_eq!($first_from, 0, "first field should start from 0");
        debug_assert_eq!($first_to, $size, "last field should matches the size of struct");
        debug_assert_eq!($first_to - $first_from, <$first_type as Field>::field_size(),
            "wrong size of field");
    }};
    ($size:expr,
     $first_name:ident : $first_type:ty [$first_from:expr => $first_to:expr],
     $($next_name:ident : $next_type:ty [$next_from:expr => $next_to:expr],)+
     ) => {{
        use $crate::encoding::Field;
        debug_assert_eq!($first_from, 0, "first field should start from 0");
        debug_assert_eq!($first_to - $first_from, <$first_type as Field>::field_size(),
            "wrong size of field");
        check_bounds!(@deep $size, $first_to,
            $($next_name : $next_type [$next_from => $next_to],)+);
    }};
    ($size:expr,) => {{
        debug_assert_eq!($size, 0, "size of empty struct should be 0");
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __ex_header_size {
    ( $($field_type:ty),* ) => {{
        #[allow(unused_mut)]
        let mut acc = 0;
        $(
            acc += <$field_type as $crate::encoding::Field>::field_size();
        )*
        acc
    }}
}

// Applies the given macro $m to all fields. $m should have the following signature:
// macro_rules! foo {
//     (
//         ($arbitrary_env),
//         $(#[$field_attr:meta])*, $field_name:ident, $field_type:ty, $from:expr, $to:expr
//     ) => { ... }
// }
#[doc(hidden)]
#[macro_export]
macro_rules! __ex_for_each_field {
    ($m:ident, ($($env:tt)*), $($fields:tt)*) => {
        __ex_for_each_field!(@inner $m ($($env)*) (0); $($fields)* );
    };

    (
        @inner $m:ident ($($env:tt)*) ($start_offset:expr);
        ($(#[$field_attr:meta])*, $field_name:ident, $field_type:ty) $($rest:tt)*
    ) => {
        $m!(
            ($($env)*),
            $(#[$field_attr])*,
            $field_name,
            $field_type,
            $start_offset,
            $start_offset + <$field_type as $crate::encoding::Field>::field_size()
        );

        __ex_for_each_field!(
            @inner $m ($($env)*)
            ($start_offset + <$field_type as $crate::encoding::Field>::field_size());
            $($rest)*
        );
    };

    (@inner $m:ident ($($env:tt)*) ($start_offset:expr);) => { };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __ex_struct_check_field {
    (
        ($latest_segment:ident, $vec:ident),
        $(#[$field_attr:meta])*, $field_name:ident, $field_type:ty, $from:expr, $to:expr
    ) => {
        let $latest_segment = <$field_type as $crate::encoding::Field>::check(
            &$vec,
            $from.into(),
            $to.into(),
            $latest_segment,
        )?;
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __ex_struct_write_field {
    (
        ($buf:ident),
        $(#[$field_attr:meta])*, $field_name:ident, $field_type:ty, $from:expr, $to:expr
    ) => {
        $crate::encoding::Field::write(&$field_name, &mut $buf, $from, $to);
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __ex_struct_mk_field {
    (
        (),
        $(#[$field_attr:meta])*, $field_name:ident, $field_type:ty, $from:expr, $to:expr
    ) => {
        $(#[$field_attr])*
        #[allow(unsafe_code)]
        pub fn $field_name(&self) -> $field_type {
            use $crate::encoding::Field;
            unsafe {
                Field::read(&self.raw, $from, $to)
            }
        }
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __ex_deserialize_field {
    (
        ($obj:ident, $writer:ident),
        $(#[$field_attr:meta])*, $field_name:ident, $field_type:ty, $from:expr, $to:expr
    ) => {
        let val = $obj.get(stringify!($field_name))
                      .ok_or("Can't get object from json.")?;
        <$field_type as ExonumJson>::deserialize_field(val, &mut $writer, $from, $to)?;
    }
}
