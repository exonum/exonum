// Copyright 2017 The Exonum Team
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

/// trait `ExonumSerializeJson` implemented for all field that allows serializing in
/// json format.
///

// TODO refer to difference between json serialization and exonum_json (ECR-156).
// TODO implement Field for float (ECR-153).
// TODO remove WriteBufferWraper hack (after refactor storage),
// should be moved into storage (ECR-156).

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::net::SocketAddr;
use std::error::Error;

use serde_json::value::Value;
use bit_vec::BitVec;
use hex::FromHex;

use crypto::{Hash, PublicKey, Signature};
use helpers::{Height, Round, ValidatorId};
use messages::RawMessage;
use encoding::{Field, Offset};
use super::WriteBufferWrapper;
// TODO: should we implement serialize for: `SecretKey`, `Seed` (ECR-156)?

macro_rules! impl_default_deserialize_owned {
    (@impl $name:ty) => {
        impl $crate::encoding::serialize::json::ExonumJsonDeserialize for $name {
            fn deserialize(value: &$crate::encoding::serialize::json::reexport::Value)
                -> Result<Self, Box<::std::error::Error>> {
                use $crate::encoding::serialize::json::reexport::from_value;
                Ok(from_value(value.clone())?)
            }
        }
    };
    ($($name:ty);*) =>
        ($(impl_default_deserialize_owned!{@impl $name})*);
}

/// `ExonumJson` is trait for object
/// that can be serialized and deserialize "in-place".
///
/// This trait is important for field types that could not be
/// deserialized directly, for example: borrowed array.
pub trait ExonumJson {
    /// write deserialized field in buffer on place.
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>>
    where
        Self: Sized;
    /// serialize field as `json::Value`
    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>>;
}

/// `ExonumJsonDeserialize` is trait for objects that could be constructed from exonum json.
pub trait ExonumJsonDeserialize {
    /// deserialize `json` value.
    fn deserialize(value: &Value) -> Result<Self, Box<Error>>
    where
        Self: Sized;
}

#[derive(Serialize, Deserialize, Debug)]
struct DurationHelper {
    secs: String,
    nanos: u32,
}
// implementation of deserialization
macro_rules! impl_deserialize_int {
    (@impl $typename:ty) => {
        impl ExonumJson for $typename {
            fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                         buffer: &mut B,
                                                         from: Offset,
                                                         to: Offset)
                -> Result<(), Box<Error>>
            {
                let number = value.as_i64().ok_or("Can't cast json as integer")?;
                buffer.write(from, to, number as $typename);
                Ok(())
            }

            fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
                Ok(Value::Number((*self).into()))
            }
        }
    };
    ($($name:ty);*) => ($(impl_deserialize_int!{@impl $name})*);
}

macro_rules! impl_deserialize_bigint {
    (@impl $typename:ty) => {
        impl ExonumJson for $typename {
            fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                        buffer: & mut B,
                                                        from: Offset,
                                                        to: Offset)
            -> Result<(), Box<Error>>
            {
                let stri = value.as_str().ok_or("Can't cast json as string")?;
                let val: $typename =  stri.parse()?;
                buffer.write(from, to, val);
                Ok(())
            }

            fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
                Ok(Value::String(self.to_string()))
            }
        }
    };
    ($($name:ty);*) => ($(impl_deserialize_bigint!{@impl $name})*);
}

macro_rules! impl_deserialize_hex_segment {
    (@impl $typename:ty) => {
        impl<'a> ExonumJson for &'a $typename {
            fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                        buffer: & mut B,
                                                        from: Offset,
                                                        to: Offset)
                -> Result<(), Box<Error>>
            {
                let stri = value.as_str().ok_or("Can't cast json as string")?;
                let val = <$typename as FromHex>:: from_hex(stri)?;
                buffer.write(from, to, &val);
                Ok(())
            }

            fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
                let hex_str = $crate::encoding::serialize::encode_hex(&self[..]);
                Ok(Value::String(hex_str))
            }
        }
    };
    ($($name:ty);*) => ($(impl_deserialize_hex_segment!{@impl $name})*);
}

impl_deserialize_int!{u8; u16; u32; i8; i16; i32}
impl_deserialize_bigint!{u64; i64}
impl_deserialize_hex_segment!{Hash; PublicKey; Signature}
impl_default_deserialize_owned!{u8; u16; u32; i8; i16; i32; u64; i64;
                                Hash; PublicKey; Signature; bool}

impl ExonumJson for bool {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let val = value.as_bool().ok_or("Can't cast json as bool")?;
        buffer.write(from, to, val);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        Ok(Value::Bool(*self))
    }
}

impl<'a> ExonumJson for &'a str {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let val = value.as_str().ok_or("Can't cast json as string")?;
        buffer.write(from, to, val);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        Ok(Value::String(self.to_string()))
    }
}

impl ExonumJson for SystemTime {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let helper: DurationHelper = ::serde_json::from_value(value.clone())?;
        let duration = Duration::new(helper.secs.parse()?, helper.nanos);
        let system_time = UNIX_EPOCH + duration;
        buffer.write(from, to, system_time);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        let duration = self.duration_since(UNIX_EPOCH)?;
        let duration = DurationHelper {
            secs: duration.as_secs().to_string(),
            nanos: duration.subsec_nanos(),
        };
        Ok(::serde_json::to_value(&duration)?)
    }
}

impl ExonumJson for SocketAddr {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let addr: SocketAddr = ::serde_json::from_value(value.clone())?;
        buffer.write(from, to, addr);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        Ok(::serde_json::to_value(&self)?)
    }
}

impl<'a> ExonumJson for &'a [Hash] {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let arr = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<Hash> = Vec::new();
        for el in arr {
            let stri = el.as_str().ok_or("Can't cast json as string")?;
            let hash = <Hash as FromHex>::from_hex(stri)?;
            vec.push(hash)
        }
        buffer.write(from, to, vec.as_slice());
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        let mut vec = Vec::new();
        for hash in self.iter() {
            vec.push(hash.serialize_field()?)
        }
        Ok(Value::Array(vec))
    }
}
impl<'a> ExonumJson for &'a [u8] {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let bytes = value.as_str().ok_or("Can't cast json as string")?;
        let arr = <Vec<u8> as FromHex>::from_hex(bytes)?;
        buffer.write(from, to, arr.as_slice());
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        Ok(Value::String(::encoding::serialize::encode_hex(self)))
    }
}

impl ExonumJson for Vec<RawMessage> {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        use messages::MessageBuffer;
        let bytes = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<_> = Vec::new();
        for el in bytes {
            let stri = el.as_str().ok_or("Can't cast json as string")?;
            let str_hex = <Vec<u8> as FromHex>::from_hex(stri)?;
            vec.push(RawMessage::new(MessageBuffer::from_vec(str_hex)));
        }
        buffer.write(from, to, vec);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        let vec = self.iter()
            .map(|slice| {
                Value::String(::encoding::serialize::encode_hex(slice))
            })
            .collect();
        Ok(Value::Array(vec))
    }
}

impl<T> ExonumJsonDeserialize for Vec<T>
where
    T: ExonumJsonDeserialize,
    for<'a> Vec<T>: Field<'a>,
{
    fn deserialize(value: &Value) -> Result<Self, Box<Error>> {
        let bytes = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<_> = Vec::new();
        for el in bytes {
            let obj = T::deserialize(el)?;
            vec.push(obj);
        }

        Ok(vec)
    }
}

// TODO remove `ExonumJsonDeserialize` needs
// after it remove impl `ExonumJsonDeserialize` for all types expect struct (ECR-156)
impl<T> ExonumJson for Vec<T>
where
    T: ExonumJsonDeserialize + ExonumJson,
    for<'a> Vec<T>: Field<'a>,
{
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let bytes = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<_> = Vec::new();
        for el in bytes {
            let obj = T::deserialize(el)?;
            vec.push(obj);
        }
        buffer.write(from, to, vec);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        let mut vec = Vec::new();
        for item in self {
            vec.push(item.serialize_field()?);
        }
        Ok(Value::Array(vec))
    }
}

impl ExonumJson for BitVec {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let stri = value.as_str().ok_or("Can't cast json as string")?;
        let mut vec = BitVec::new();
        for (i, ch) in stri.chars().enumerate() {
            let val = if ch == '1' {
                true
            } else if ch == '0' {
                false
            } else {
                Err(format!("BitVec should contain only 0 or 1, not {}", ch))?
            };
            vec.set(i, val);
        }
        buffer.write(from, to, vec);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        let mut out = String::new();
        for i in self.iter() {
            if i {
                out.push('1');
            } else {
                out.push('0');
            }
        }
        Ok(Value::String(out))
    }
}

// TODO: Make a macro for tuple struct typedefs (ECR-154)?
impl ExonumJson for Height {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let val: u64 = value.as_str().ok_or("Can't cast json as string")?.parse()?;
        buffer.write(from, to, Height(val));
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        let val: u64 = self.to_owned().into();
        Ok(Value::String(val.to_string()))
    }
}

impl ExonumJson for Round {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let number = value.as_i64().ok_or("Can't cast json as integer")?;
        buffer.write(from, to, Round(number as u32));
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        let val: u32 = self.to_owned().into();
        Ok(Value::Number(val.into()))
    }
}

impl ExonumJson for ValidatorId {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let number = value.as_i64().ok_or("Can't cast json as integer")?;
        buffer.write(from, to, ValidatorId(number as u16));
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        let val: u16 = self.to_owned().into();
        Ok(Value::Number(val.into()))
    }
}

/// Reexport of `serde` specific traits, this reexports
/// provide compatibility layer with important `serde_json` version.
pub mod reexport {
    pub use serde_json::{from_str, from_value, to_string, to_value, Value};
    pub use serde_json::map::Map;
}
