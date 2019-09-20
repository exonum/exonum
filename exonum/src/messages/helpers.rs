use hex::{FromHex, ToHex};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use std::fmt::Display;

use super::Signed;

/// Uses `ToHex`/`FromHex` to serialize arbitrary type `T` as
/// hexadecimal string rather than real Serde::serialize.
pub(crate) struct HexStringRepresentation;

impl HexStringRepresentation {
    pub(crate) fn serialize<S, T>(message: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: ToHex,
    {
        let hex_string = message.encode_hex::<String>();
        <String as Serialize>::serialize(&hex_string, serializer)
    }

    pub(crate) fn deserialize<'a, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'a>,
        T: FromHex,
        <T as FromHex>::Error: Display,
    {
        let hex_string = <String as Deserialize>::deserialize(deserializer)?;
        FromHex::from_hex(&hex_string).map_err(de::Error::custom)
    }
}

/// Returns hexadecimal string representation of `message`.
pub fn to_hex_string<T>(message: &Signed<T>) -> String {
    let hex_string = message.encode_hex::<String>();
    hex_string
}
