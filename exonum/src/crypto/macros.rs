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

//! Common macros for crypto module.

macro_rules! implement_public_sodium_wrapper {
    ($(#[$attr:meta])* struct $name:ident, $name_from:ident, $size:expr) => (
    #[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash, Serialize, Deserialize)]
    $(#[$attr])*
    pub struct $name($name_from);

    impl $name {
        /// Creates a new instance filled with zeros.
        pub fn zero() -> Self {
            $name::new([0; $size])
        }
    }

    impl $name {
        /// Creates a new instance from bytes array.
        pub fn new(bytes_array: [u8; $size]) -> Self {
            $name($name_from(bytes_array))
        }

        /// Creates a new instance from bytes slice.
        pub fn from_slice(bytes_slice: &[u8]) -> Option<Self> {
            $name_from::from_slice(bytes_slice).map($name)
        }

        /// Returns a hex representation of binary data.
        /// Lower case letters are used (e.g. f9b4ca).
        pub fn to_hex(&self) -> String {
            encode_hex(self)
        }
    }

    impl AsRef<[u8]> for $name {
        fn as_ref(&self) -> &[u8] {
            self.0.as_ref()
        }
    }

    impl FromStr for $name {
        type Err = FromHexError;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            $name::from_hex(s)
        }
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, stringify!($name))?;
            write!(f, "(")?;
            for i in &self[0..BYTES_IN_DEBUG] {
                write!(f, "{:02X}", i)?
            }
            write!(f, ")")
        }
    }

    impl fmt::Display for $name {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str(&self.to_hex())
        }
    }
    implement_from_hex!($name);
    )
}

macro_rules! implement_private_sodium_wrapper {
    ($(#[$attr:meta])* struct $name:ident, $name_from:ident, $size:expr) => (
    #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
    $(#[$attr])*
    pub struct $name($name_from);

    impl $name {
        /// Creates a new instance filled with zeros.
        pub fn zero() -> Self {
            $name::new([0; $size])
        }
    }

    impl $name {
        /// Creates a new instance from bytes array.
        pub fn new(bytes_array: [u8; $size]) -> Self {
            $name($name_from(bytes_array))
        }

        /// Creates a new instance from bytes slice.
        pub fn from_slice(bytes_slice: &[u8]) -> Option<Self> {
            $name_from::from_slice(bytes_slice).map($name)
        }

        /// Returns a hex representation of binary data.
        /// Lower case letters are used (e.g. f9b4ca).
        pub fn to_hex(&self) -> String {
            encode_hex(&self[..])
        }
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, stringify!($name))?;
            write!(f, "(")?;
            for i in &self[0..BYTES_IN_DEBUG] {
                write!(f, "{:02X}", i)?
            }
            write!(f, "...)")
        }
    }

    impl ToHex for $name {
        fn write_hex<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
            (self.0).0.as_ref().write_hex(w)
        }

        fn write_hex_upper<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
            (self.0).0.as_ref().write_hex_upper(w)
        }
    }
    implement_from_hex!($name);
    )
}

macro_rules! implement_from_hex {
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
