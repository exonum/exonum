use hex::{FromHex, ToHex};
use serde::{de, ser, Deserialize, Deserializer, Serialize, Serializer};

use std::fmt::Display;

use super::Signed;

// TODO use hex-buffer-serde [ECR-3222]

/// Uses `ToHex`/`FromHex` to serialize arbitrary type `T` as
/// hexadecimal string rather than real Serde::serialize.
pub(crate) struct HexStringRepresentation;

impl HexStringRepresentation {
    pub(crate) fn serialize<S, T>(message: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: ToHex,
    {
        let mut hex_string = String::new();
        message
            .write_hex(&mut hex_string)
            .map_err(ser::Error::custom)?;
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
    let mut hex_string = String::new();
    message.write_hex(&mut hex_string).unwrap();
    hex_string
}
