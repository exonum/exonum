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

//! Common macros for crypto module.

macro_rules! implement_public_crypto_wrapper {
    ($(#[$attr:meta])* struct $name:ident, $size:expr) => (
    #[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
    $(#[$attr])*
    pub struct $name($crate::crypto_impl::$name);

    impl $name {
        /// Creates a new instance filled with zeros.
        pub fn zero() -> Self {
            $name::new([0; $size])
        }
    }

    impl $name {
        /// Creates a new instance from bytes array.
        pub fn new(bytes_array: [u8; $size]) -> Self {
            $name($crate::crypto_impl::$name(bytes_array))
        }

        /// Creates a new instance from bytes slice.
        pub fn from_slice(bytes_slice: &[u8]) -> Option<Self> {
            $crate::crypto_impl::$name::from_slice(bytes_slice).map($name)
        }

        /// Copies bytes from this instance.
        pub fn as_bytes(&self) -> [u8; $size] {
            (self.0).0
        }

        /// Returns a hex representation of binary data.
        /// Lower case letters are used (e.g. `f9b4ca`).
        pub fn to_hex(&self) -> String {
            encode_hex(self)
        }
    }

    impl AsRef<[u8]> for $name {
        fn as_ref(&self) -> &[u8] {
            self.0.as_ref()
        }
    }

    impl Default for $name {
        fn default() -> Self {
            Self::zero()
        }
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, stringify!($name))?;
            write!(f, "(")?;
            write_short_hex(f, &self[..])?;
            write!(f, ")")
        }
    }

    impl fmt::Display for $name {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write_short_hex(f, &self[..])
        }
    }
    )
}

macro_rules! implement_private_crypto_wrapper {
    ($(#[$attr:meta])* struct $name:ident, $size:expr) => (
    #[derive(Clone, PartialEq, Eq)]
    $(#[$attr])*
    pub struct $name($crate::crypto_impl::$name);

    impl $name {
        /// Creates a new instance filled with zeros.
        pub fn zero() -> Self {
            $name::new([0; $size])
        }
    }

    impl $name {
        /// Creates a new instance from bytes array.
        pub fn new(bytes_array: [u8; $size]) -> Self {
            $name($crate::crypto_impl::$name(bytes_array))
        }

        /// Creates a new instance from bytes slice.
        pub fn from_slice(bytes_slice: &[u8]) -> Option<Self> {
            $crate::crypto_impl::$name::from_slice(bytes_slice).map($name)
        }

        /// Returns a hex representation of binary data.
        /// Lower case letters are used (e.g. f9b4ca).
        pub fn to_hex(&self) -> String {
            encode_hex(&self[..])
        }
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, stringify!($name))?;
            write!(f, "(")?;
            write_short_hex(f, &self[..])?;
            write!(f, ")")
        }
    }

    impl Default for $name {
        fn default() -> Self {
            Self::zero()
        }
    }

    impl ToHex for $name {
        fn encode_hex<T: std::iter::FromIterator<char>>(&self) -> T {
            (self.0).0.as_ref().encode_hex()
        }

        fn encode_hex_upper<T: std::iter::FromIterator<char>>(&self) -> T {
            (self.0).0.as_ref().encode_hex_upper()
        }
    }
    )
}

macro_rules! implement_serde {
    ($name:ident) => {
        impl FromHex for $name {
            type Error = FromHexError;

            fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Self::Error> {
                let bytes = Vec::<u8>::from_hex(v)?;
                if let Some(self_value) = Self::from_slice(bytes.as_ref()) {
                    Ok(self_value)
                } else {
                    Err(FromHexError::InvalidStringLength)
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let hex_string = encode_hex(&self[..]);
                ser.serialize_str(&hex_string)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct HexVisitor;

                impl<'v> Visitor<'v> for HexVisitor {
                    type Value = $name;
                    fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                        write!(fmt, "expecting str.")
                    }
                    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                    where
                        E: de::Error,
                    {
                        $name::from_hex(s).map_err(|_| de::Error::custom("Invalid hex"))
                    }
                }
                deserializer.deserialize_str(HexVisitor)
            }
        }
    };
}

macro_rules! implement_index_traits {
    ($new_type:ident) => {
        impl Index<Range<usize>> for $new_type {
            type Output = [u8];
            fn index(&self, _index: Range<usize>) -> &[u8] {
                let inner = &self.0;
                inner.0.index(_index)
            }
        }
        impl Index<RangeTo<usize>> for $new_type {
            type Output = [u8];
            fn index(&self, _index: RangeTo<usize>) -> &[u8] {
                let inner = &self.0;
                inner.0.index(_index)
            }
        }
        impl Index<RangeFrom<usize>> for $new_type {
            type Output = [u8];
            fn index(&self, _index: RangeFrom<usize>) -> &[u8] {
                let inner = &self.0;
                inner.0.index(_index)
            }
        }
        impl Index<RangeFull> for $new_type {
            type Output = [u8];
            fn index(&self, _index: RangeFull) -> &[u8] {
                let inner = &self.0;
                inner.0.index(_index)
            }
        }
    };
}
