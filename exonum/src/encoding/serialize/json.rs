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

use bit_vec::BitVec;
use chrono::{DateTime, Duration, TimeZone, Utc};
use hex::FromHex;
use rust_decimal::Decimal;
/// trait `ExonumSerializeJson` implemented for all field that allows serializing in json format.
use serde_json::{self, value::Value};
use uuid::Uuid;

use std::{error::Error, net::SocketAddr};

use super::WriteBufferWrapper;
use crypto::{Hash, PublicKey, Signature};
use encoding::{Field, Offset};
use helpers::{Height, Round, ValidatorId};
use messages::RawMessage;

// TODO: Should we implement serialize for: `SecretKey`, `Seed`. (ECR-156)

macro_rules! impl_default_deserialize_owned {
    (@impl $name:ty) => {
        impl $crate::encoding::serialize::json::ExonumJsonDeserialize for $name {
            fn deserialize(value: &$crate::encoding::serialize::json::reexport::Value)
                -> Result<Self, Box<dyn (::std::error::Error)>> {
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
    ) -> Result<(), Box<dyn Error>>
    where
        Self: Sized;
    /// serialize field as `json::Value`
    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>>;
}

/// `ExonumJsonDeserialize` is trait for objects that could be constructed from exonum json.
pub trait ExonumJsonDeserialize {
    /// deserialize `json` value.
    fn deserialize(value: &Value) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;
}

#[derive(Serialize, Deserialize, Debug)]
struct TimestampHelper {
    secs: String,
    nanos: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct DurationHelper {
    secs: String,
    nanos: i32,
}

// implementation of deserialization
macro_rules! impl_deserialize_int {
    (@impl $typename:ty) => {
        impl ExonumJson for $typename {
            fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                         buffer: &mut B,
                                                         from: Offset,
                                                         to: Offset)
                -> Result<(), Box<dyn Error>>
            {
                let number = value.as_i64().ok_or("Can't cast json as integer")?;
                buffer.write(from, to, number as $typename);
                Ok(())
            }

            fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
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
            -> Result<(), Box<dyn Error>>
            {
                let string = value.as_str().ok_or("Can't cast json as string")?;
                let val: $typename =  string.parse()?;
                buffer.write(from, to, val);
                Ok(())
            }

            fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
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
                -> Result<(), Box<dyn Error>>
            {
                let string = value.as_str().ok_or("Can't cast json as string")?;
                let val = <$typename as FromHex>:: from_hex(string)?;
                buffer.write(from, to, &val);
                Ok(())
            }

            fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
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
impl_default_deserialize_owned!{u8; u16; u32; i8; i16; i32; u64; i64}
impl_default_deserialize_owned!{Hash; PublicKey; Signature; bool}

impl ExonumJson for bool {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<dyn Error>> {
        let val = value.as_bool().ok_or("Can't cast json as bool")?;
        buffer.write(from, to, val);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
        Ok(Value::Bool(*self))
    }
}

impl<'a> ExonumJson for &'a str {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<dyn Error>> {
        let val = value.as_str().ok_or("Can't cast json as string")?;
        buffer.write(from, to, val);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
        Ok(Value::String(self.to_string()))
    }
}

impl ExonumJson for DateTime<Utc> {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<dyn Error>> {
        let helper: TimestampHelper = serde_json::from_value(value.clone())?;
        let date_time = Utc.timestamp(helper.secs.parse()?, helper.nanos);
        buffer.write(from, to, date_time);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
        let timestamp = TimestampHelper {
            secs: self.timestamp().to_string(),
            nanos: self.timestamp_subsec_nanos(),
        };
        Ok(serde_json::to_value(&timestamp)?)
    }
}

impl ExonumJson for Duration {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<dyn Error>> {
        let helper: DurationHelper = serde_json::from_value(value.clone())?;
        let seconds = helper.secs.parse()?;

        let seconds_duration = Self::seconds(seconds);
        let nanos_duration = Self::nanoseconds(i64::from(helper.nanos));

        let result = seconds_duration.checked_add(&nanos_duration);
        match result {
            Some(duration) => {
                buffer.write(from, to, duration);
                Ok(())
            }
            None => Err(format!(
                "Can't deserialize Duration: {} secs, {} nanos",
                seconds, helper.nanos
            ))?,
        }
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
        let secs = self.num_seconds();
        let nanos_as_duration = *self - Self::seconds(secs);
        // Since we're working with only nanos, no overflow is expected here.
        let nanos = nanos_as_duration.num_nanoseconds().unwrap() as i32;

        let timestamp = DurationHelper {
            secs: secs.to_string(),
            nanos,
        };
        Ok(serde_json::to_value(&timestamp)?)
    }
}

impl ExonumJson for SocketAddr {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<dyn Error>> {
        let addr: Self = serde_json::from_value(value.clone())?;
        buffer.write(from, to, addr);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
        Ok(serde_json::to_value(&self)?)
    }
}

impl<'a> ExonumJson for &'a [Hash] {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<dyn Error>> {
        let arr = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<Hash> = Vec::new();
        for el in arr {
            let string = el.as_str().ok_or("Can't cast json as string")?;
            let hash = <Hash as FromHex>::from_hex(string)?;
            vec.push(hash)
        }
        buffer.write(from, to, vec.as_slice());
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
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
    ) -> Result<(), Box<dyn Error>> {
        let bytes = value.as_str().ok_or("Can't cast json as string")?;
        let arr = <Vec<u8> as FromHex>::from_hex(bytes)?;
        buffer.write(from, to, arr.as_slice());
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
        Ok(Value::String(::encoding::serialize::encode_hex(self)))
    }
}

impl ExonumJson for Vec<RawMessage> {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<dyn Error>> {
        use messages::MessageBuffer;
        let bytes = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<_> = Vec::new();
        for el in bytes {
            let string = el.as_str().ok_or("Can't cast json as string")?;
            let str_hex = <Vec<u8> as FromHex>::from_hex(string)?;
            vec.push(RawMessage::new(MessageBuffer::from_vec(str_hex)));
        }
        buffer.write(from, to, vec);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
        let vec = self.iter()
            .map(|slice| Value::String(::encoding::serialize::encode_hex(slice)))
            .collect();
        Ok(Value::Array(vec))
    }
}

impl<T> ExonumJsonDeserialize for Vec<T>
where
    T: ExonumJsonDeserialize,
    for<'a> Vec<T>: Field<'a>,
{
    fn deserialize(value: &Value) -> Result<Self, Box<dyn Error>> {
        let bytes = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<_> = Vec::new();
        for el in bytes {
            let obj = T::deserialize(el)?;
            vec.push(obj);
        }

        Ok(vec)
    }
}

// TODO: Remove `ExonumJsonDeserialize` needs
// after it remove impl `ExonumJsonDeserialize` for all types expect struct. (ECR-156)
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
    ) -> Result<(), Box<dyn Error>> {
        let bytes = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<_> = Vec::new();
        for el in bytes {
            let obj = T::deserialize(el)?;
            vec.push(obj);
        }
        buffer.write(from, to, vec);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
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
    ) -> Result<(), Box<dyn Error>> {
        let string = value.as_str().ok_or("Can't cast json as string")?;
        let mut vec = Self::new();
        for (i, ch) in string.chars().enumerate() {
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

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
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

// TODO: Make a macro for tuple struct type definitions? (ECR-154)
impl ExonumJson for Height {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<dyn Error>> {
        let val: u64 = value.as_str().ok_or("Can't cast json as string")?.parse()?;
        buffer.write(from, to, Height(val));
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
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
    ) -> Result<(), Box<dyn Error>> {
        let number = value.as_i64().ok_or("Can't cast json as integer")?;
        buffer.write(from, to, Round(number as u32));
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
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
    ) -> Result<(), Box<dyn Error>> {
        let number = value.as_i64().ok_or("Can't cast json as integer")?;
        buffer.write(from, to, ValidatorId(number as u16));
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
        let val: u16 = self.to_owned().into();
        Ok(Value::Number(val.into()))
    }
}

impl ExonumJson for Uuid {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<dyn Error>> {
        let uuid: Self = serde_json::from_value(value.clone())?;
        buffer.write(from, to, uuid);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
        Ok(serde_json::to_value(&self)?)
    }
}

impl ExonumJson for Decimal {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<dyn Error>> {
        let decimal: Self = serde_json::from_value(value.clone())?;
        buffer.write(from, to, decimal);
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<dyn Error + Send + Sync>> {
        Ok(serde_json::to_value(&self)?)
    }
}

/// Reexport of `serde` specific traits, this reexports
/// provide compatibility layer with important `serde_json` version.
pub mod reexport {
    pub use serde_json::map::Map;
    pub use serde_json::{from_str, from_value, to_string, to_value, Error, Value};
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]

    use super::*;
    use encoding::CheckedOffset;

    #[test]
    fn exonum_json_for_duration_round_trip() {
        let durations = [
            Duration::zero(),
            Duration::max_value(),
            Duration::min_value(),
            Duration::nanoseconds(999_999_999),
            Duration::nanoseconds(-999_999_999),
            Duration::seconds(42) + Duration::nanoseconds(15),
            Duration::seconds(-42) + Duration::nanoseconds(-15),
        ];

        // Variables for serialization/deserialization
        let mut buffer = vec![0; Duration::field_size() as usize];
        let from: Offset = 0;
        let to: Offset = Duration::field_size();
        let checked_from = CheckedOffset::new(from);
        let checked_to = CheckedOffset::new(to);

        for duration in durations.iter() {
            let serialized = duration
                .serialize_field()
                .expect("Can't serialize duration");

            Duration::deserialize_field(&serialized, &mut buffer, from, to)
                .expect("Can't deserialize duration");

            Duration::check(&buffer, checked_from, checked_to, checked_to)
                .expect("Incorrect result of deserialization");

            let result_duration;

            unsafe {
                result_duration = Duration::read(&buffer, from, to);
            }

            assert_eq!(*duration, result_duration);
        }
    }

}
