// Copyright 2020 The Exonum Team
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

//! Macros useful for work with types that implement `BinaryKey` and `BinaryValue` traits.

/// Fast concatenation of byte arrays and/or keys that implements
/// `BinaryKey` trait.
///
/// ```
/// let prefix = vec![0_u8; 10];
/// let key = PublicKey::zero();
///
/// let _result = concat_keys!(prefix, key);
/// ```
macro_rules! concat_keys {
    (@capacity $key:expr) => ( $key.size() );
    (@capacity $key:expr, $($tail:expr),+) => (
        BinaryKey::size($key) + concat_keys!(@capacity $($tail),+)
    );
    ($($key:expr),+) => ({
        let capacity = concat_keys!(@capacity $($key),+);

        let mut buf = vec![0; capacity];
        let mut _pos = 0;
        $(
            _pos += BinaryKey::write($key, &mut buf[_pos.._pos + BinaryKey::size($key)]);
        )*
        buf
    });
}

/// Implement `ObjectHash` trait for any type that implements `BinaryValue`.
#[macro_export]
macro_rules! impl_object_hash_for_binary_value {
    ($( $type:ty ),*) => {
        $(
            impl ObjectHash for $type {
                fn object_hash(&self) -> Hash {
                    exonum_crypto::hash(&self.to_bytes())
                }
            }
        )*
    };
}

// Think about bincode instead of protobuf. [ECR-3222]
/// Implements `BinaryKey` trait for any type that implements `BinaryValue`.
#[macro_export]
macro_rules! impl_binary_key_for_binary_value {
    ($type:ty) => {
        impl exonum_merkledb::BinaryKey for $type {
            fn size(&self) -> usize {
                exonum_merkledb::BinaryValue::to_bytes(self).len()
            }

            fn write(&self, buffer: &mut [u8]) -> usize {
                let mut bytes = exonum_merkledb::BinaryValue::to_bytes(self);
                buffer.swap_with_slice(&mut bytes);
                bytes.len()
            }

            fn read(buffer: &[u8]) -> Self::Owned {
                // `unwrap` is safe because only this code uses for
                // serialize and deserialize these keys.
                <Self as exonum_merkledb::BinaryValue>::from_bytes(buffer.into()).unwrap()
            }
        }
    };
}

/// Hex conversions for the given `BinaryValue`.
///
/// Implements `hex::FromHex` and `hex::ToHex` conversions for the given `BinaryValue` and uses them in
/// the implementation of the following traits:
///
/// `FromStr`, `Display`, `Serialize`, `Deserialize`.
///
/// Pay attention that macro uses `serde_str` under the hood.
#[macro_export]
macro_rules! impl_serde_hex_for_binary_value {
    ($name:ident) => {
        impl hex::ToHex for $name {
            fn encode_hex<T: std::iter::FromIterator<char>>(&self) -> T {
                use exonum_merkledb::BinaryValue;

                BinaryValue::to_bytes(self).encode_hex()
            }

            fn encode_hex_upper<T: std::iter::FromIterator<char>>(&self) -> T {
                use exonum_merkledb::BinaryValue;

                BinaryValue::to_bytes(self).encode_hex_upper()
            }
        }

        impl hex::FromHex for $name {
            type Error = failure::Error;

            fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Self::Error> {
                use exonum_merkledb::BinaryValue;

                let bytes = Vec::<u8>::from_hex(v)?;
                <Self as BinaryValue>::from_bytes(bytes.into()).map_err(From::from)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                use hex::ToHex;

                write!(f, "{}", <Self as ToHex>::encode_hex::<String>(self))
            }
        }

        impl std::str::FromStr for $name {
            type Err = failure::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                use hex::FromHex;

                <Self as FromHex>::from_hex(s)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                serde_str::deserialize(deserializer)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serde_str::serialize(self, serializer)
            }
        }
    };
}

/// Implements `BinaryValue` traint for tuples cross single member generic tuple struct.
/// If expression begins with for keyword it creates implementation for existend structure
/// supposing it meets requirements, mentioned above. Otherwise it creates new structure
/// according to supplied meta attributes, visibility, name and type constraint for generic type.
/// Target tuple types must be listed in curly braces.
#[macro_export]
macro_rules! binary_value_tuple_impls {
    () => ();
    ($(#[$attr:meta])* $vis:vis $name:ident {$(($t1:ty$(, $t2:ty$(, $t3:ty$(, $t4:ty$(, $t5:ty$(, $t6:ty$(, $t7:ty$(, $t8:ty$(, $t9:ty$(, $t10:ty$(, $t11:ty$(, $t12:ty)?)?)?)?)?)?)?)?)?)?)?$(,)?)),+$(,)?} $($rest:tt)*) => {
        $(#[$attr])*
        $vis struct $name<T>(pub T);
        $crate::binary_value_tuple_impls!(for $name {$(($t1$(, $t2$(, $t3$(, $t4$(, $t5$(, $t6$(, $t7$(, $t8$(, $t9$(, $t10$(, $t11$(, $t12)?)?)?)?)?)?)?)?)?)?)?)),+} $($rest)*);
    };
    (for $name:ident {$(($t1:ty$(, $t2:ty$(, $t3:ty$(, $t4:ty$(, $t5:ty$(, $t6:ty$(, $t7:ty$(, $t8:ty$(, $t9:ty$(, $t10:ty$(, $t11:ty$(, $t12:ty)?)?)?)?)?)?)?)?)?)?)?$(,)?)),+$(,)?} $($rest:tt)*) => {
        $($crate::binary_value_tuple_impls!($name ($t1, 0)$(, ($t2, 1)$(, ($t3, 2)$(, ($t4, 3)$(, ($t5, 4)$(, ($t6, 5)$(, ($t7, 6)$(, ($t8, 7)$(, ($t9, 8)$(, ($t10, 9)$(, ($t11, 10)$(, ($t12, 11))?)?)?)?)?)?)?)?)?)?)?);)+
        $crate::binary_value_tuple_impls!($($rest)*);
    };
    ($name:ident $(($t:ty, $idx:tt)),+) => {
        impl $crate::BinaryValue for $name<($($t,)+)> {
            fn to_bytes(&self) -> Vec<u8> {
                let inner = &self.0;
                $crate::concat_buffers(&mut [$(inner.$idx.to_bytes()),+])
            }

            fn from_bytes(bytes: ::std::borrow::Cow<'_, [u8]>) -> Result<Self, failure::Error> {
                use ::std::borrow::Cow;
                let parts = $crate::split_buffer_into_sized_parts(&bytes, $crate::binary_value_tuple_impls!(@COUNT $($t)+))?;

                Ok($name((
                    $(<$t as $crate::BinaryValue>::from_bytes(Cow::Borrowed(&parts[$idx]))?),+)
                ))
            }
        }

        impl From<($($t,)+)> for $name<($($t,)+)> {
            fn from(t: ($($t,)+)) -> Self {
                Self(t)
            }
        }
    };
    (@COUNT $head:tt $($tail:tt)*) => (1_usize + $crate::binary_value_tuple_impls!(@COUNT $($tail)*));
    (@COUNT) => (0_usize);
}
