// Copyright 2019 The Exonum Team
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

                self.to_bytes().encode_hex()
            }

            fn encode_hex_upper<T: std::iter::FromIterator<char>>(&self) -> T {
                use exonum_merkledb::BinaryValue;

                self.to_bytes().encode_hex_upper()
            }
        }

        impl hex::FromHex for $name {
            type Error = failure::Error;

            fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Self::Error> {
                use exonum_merkledb::BinaryValue;

                let bytes = Vec::<u8>::from_hex(v)?;
                Self::from_bytes(bytes.into()).map_err(From::from)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                use hex::ToHex;

                write!(f, "{}", self.encode_hex::<String>())
            }
        }

        impl std::str::FromStr for $name {
            type Err = failure::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                use hex::FromHex;

                Self::from_hex(s)
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
