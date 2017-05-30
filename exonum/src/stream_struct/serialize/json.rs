/// trait `ExonumSerializeJson` implemented for all field that allows serializing in
/// json format.
///

//\TODO refer to difference between json serialization and exonum_json
//\TODO implement Field for float
//\TODO remove WriteBufferWraper hack (after refactor storage), should be moved into storage
//\TODO implement owned ExonumJsonDeserialize for big ints

use serde::{Serializer, Serialize};

use serde_json::value::Value;
use bit_vec::BitVec;
use hex::ToHex;

use std::time::{SystemTime, Duration, UNIX_EPOCH};
use std::sync::Arc;
use std::net::SocketAddr;
use std::error::Error;

use crypto::{Hash, PublicKey, SecretKey, Seed, Signature};

use stream_struct::{Field, Offset};
use super::HexValue;
use super::WriteBufferWrapper;

macro_rules! impl_default_deserialize_owned {
    (@impl $name:ty) => {
        impl $crate::stream_struct::serialize::json::ExonumJsonDeserialize for $name {
            fn deserialize(value: &$crate::stream_struct::serialize::json::reexport::Value)
                                                        -> Result<Self, Box<::std::error::Error>> {
                use $crate::stream_struct::serialize::json::reexport::from_value;
                Ok(from_value(value.clone())?)
            }
        }
    };
    ($($name:ty);*) =>
        ($(impl_default_deserialize_owned!{@impl $name})*);
}

macro_rules! propagate_deserialize_to_owned {
    (@impl $name:ty) => {
        impl $crate::stream_struct::serialize::json::ExonumJsonDeserializeField for $name {
            fn deserialize_field<B: WriteBufferWrapper>(
                value: &$crate::stream_struct::serialize::json::reexport::Value,
                                                        buffer: &mut B,
                                                        from: Offset,
                                                        to: Offset)
                                                        -> Result<(), Box<::std::error::Error>> {
                use $crate::stream_struct::serialize::json::ExonumJsonDeserialize;
                // deserialize full field
                let structure = <Self as ExonumJsonDeserialize>::deserialize(value)?;
                // then write it
                buffer.write(from, to, structure);
                Ok(())
            }
        }

    };
    ($($name:ty);*) =>
        ($(propagate_deserialize_to_owned!{@impl $name})*);
}

/// `ExonumJsonDeserializeField` is trait for object
/// that can be serialized "in-place" of storage structure.
///
/// This trait important for field types that could not be
/// deserialized directly, for example: borrowed array.
pub trait ExonumJsonDeserializeField {
    /// write deserialized field in buffer on place.
    fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                buffer: &mut B,
                                                from: Offset,
                                                to: Offset)
                                                -> Result<(), Box<Error>>;
}

/// `ExonumJsonDeserialize` is trait for objects that could be constructed from exonum json.
pub trait ExonumJsonDeserialize {
    /// deserialize `json` value.
    fn deserialize(value: &Value) -> Result<Self, Box<Error>> where Self: Sized;
}

/// `ExonumJsonSerialize` is trait for object that
/// could be serialized as json with exonum protocol specific aspects.
pub trait ExonumJsonSerialize {
    /// serialize value with specific apsects.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
}

/// Helper function, for wrapping value that should be serialized as `ExonumJsonSerialize`
pub fn wrap<T: ExonumJsonSerialize>(val: &T) -> ExonumJsonSerializeWrapper<T> {
    ExonumJsonSerializeWrapper(val)
}

/// Wrapping struct that allows implementing custom serializing aspects in json.
#[derive(Debug)]
pub struct ExonumJsonSerializeWrapper<'a, T: ExonumJsonSerialize + 'a>(&'a T);

#[derive(Serialize, Deserialize, Debug)]
struct DurationHelper {
    secs: String,
    nanos: u32,
}

impl<'a, T: ExonumJsonSerialize + 'a> Serialize for ExonumJsonSerializeWrapper<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        self.0.serialize(serializer)
    }
}

impl_default_serialize!{ExonumJsonSerialize =>
    u8; u16; u32; i8; i16; i32; bool;
    Signature; SocketAddr}

impl_default_serialize_deref!{ExonumJsonSerialize =>
    Hash; PublicKey; SecretKey; Seed}

impl ExonumJsonSerialize for u64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl ExonumJsonSerialize for i64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl<'a> ExonumJsonSerialize for &'a [u8] {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'a> ExonumJsonSerialize for &'a str {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self)
    }
}

impl ExonumJsonSerialize for BitVec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = String::new();
        for i in self.iter() {
            if i {
                out.push('1');
            } else {
                out.push('0');
            }
        }
        serializer.serialize_str(&out)
    }
}

impl ExonumJsonSerialize for Arc<::messages::MessageBuffer> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let slice = self.as_ref();
        serializer.serialize_str(&slice.to_hex())
    }
}

impl<'a> ExonumJsonSerialize for &'a [Hash] {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let vec: Vec<_> = self.iter().collect();
        serializer.collect_seq(vec.iter().map(|v| wrap(v)))
    }
}

impl<T> ExonumJsonSerialize for Vec<T>
    where T: ExonumJsonSerialize
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter().map(|v| wrap(v)))
    }
}

impl ExonumJsonSerialize for SystemTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let duration = self.duration_since(UNIX_EPOCH).map_err(S::Error::custom)?;
        let duration = DurationHelper {
            secs: duration.as_secs().to_string(),
            nanos: duration.subsec_nanos(),
        };
        <DurationHelper as Serialize>::serialize(&duration, serializer)
    }
}

// implementation of deserialization

macro_rules! impl_deserialize_int {
    (@impl $typename:ty) => {
        impl ExonumJsonDeserializeField for $typename {
            fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                         buffer: &mut B,
                                                         from: Offset,
                                                         to: Offset )
                -> Result<(), Box<Error>>
            {
                let number = value.as_i64().ok_or("Can't cast json as integer")?;
                buffer.write(from, to, number as $typename);
                Ok(())
            }
        }
    };
    ($($name:ty);*) => ($(impl_deserialize_int!{@impl $name})*);
}

macro_rules! impl_deserialize_bigint {
    (@impl $typename:ty) => {
        impl ExonumJsonDeserializeField for $typename {
            fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                        buffer: & mut B,
                                                        from: Offset,
                                                        to: Offset )
            -> Result<(), Box<Error>>
            {
                let stri = value.as_str().ok_or("Can't cast json as string")?;
                let val: $typename =  stri.parse()?;
                buffer.write(from, to, val);
                Ok(())
            }
        }
    };
    ($($name:ty);*) => ($(impl_deserialize_bigint!{@impl $name})*);
}

/*
macro_rules! impl_deserialize_float {
    (@impl $traitname:ident $typename:ty) => {
        impl<'a> ExonumJsonDeserialize for $typename {
            fn deserialize(value: &Value, buffer: &'a mut Vec<u8>,
                            from: usize, to: usize ) -> bool {
                    value.as_f64()
                         .map(|v| v as $typename)
                         .map(|val| val.write(buffer, from, to))
                         .is_some()
            }
        }
    };
    ( $($name:ty);*) => ($(impl_deserialize_float!{@impl  $name})*);
}
impl_deserialize_int!{ f32; f64 }
*/

macro_rules! impl_deserialize_hex_segment {
    (@impl $typename:ty) => {
        impl<'a> ExonumJsonDeserializeField for &'a $typename {
            fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                        buffer: & mut B,
                                                        from: Offset,
                                                        to: Offset )
                -> Result<(), Box<Error>>
            {
                let stri = value.as_str().ok_or("Can't cast json as string")?;
                let val = <$typename as HexValue>:: from_hex(stri)?;
                buffer.write(from, to, &val);
                Ok(())
            }
        }
    };
    ($($name:ty);*) => ($(impl_deserialize_hex_segment!{@impl $name})*);
}

impl_deserialize_int!{u8; u16; u32; i8; i16; i32 }
impl_deserialize_bigint!{u64; i64}

impl_deserialize_hex_segment!{Hash; PublicKey; Signature}

impl_default_deserialize_owned!{u8; u16; u32; i8; i16; i32; u64; i64;
                                Hash; PublicKey; Signature; bool}

impl ExonumJsonDeserializeField for bool {
    fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                buffer: &mut B,
                                                from: Offset,
                                                to: Offset)
                                                -> Result<(), Box<Error>> {
        let val = value.as_bool().ok_or("Can't cast json as bool")?;
        buffer.write(from, to, val);
        Ok(())
    }
}

impl<'a> ExonumJsonDeserializeField for &'a str {
    fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                buffer: &mut B,
                                                from: Offset,
                                                to: Offset)
                                                -> Result<(), Box<Error>> {
        let val = value.as_str().ok_or("Can't cast json as string")?;
        buffer.write(from, to, val);
        Ok(())
    }
}

impl ExonumJsonDeserializeField for SystemTime {
    fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                buffer: &mut B,
                                                from: Offset,
                                                to: Offset)
                                                -> Result<(), Box<Error>> {
        let helper: DurationHelper = ::serde_json::from_value(value.clone())?;
        let duration = Duration::new(helper.secs.parse()?, helper.nanos);
        let system_time = UNIX_EPOCH + duration;
        buffer.write(from, to, system_time);
        Ok(())
    }
}

impl ExonumJsonDeserializeField for SocketAddr {
    fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                buffer: &mut B,
                                                from: Offset,
                                                to: Offset)
                                                -> Result<(), Box<Error>> {
        let addr: SocketAddr = ::serde_json::from_value(value.clone())?;
        buffer.write(from, to, addr);
        Ok(())
    }
}

impl<'a> ExonumJsonDeserializeField for &'a [Hash] {
    fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                buffer: &mut B,
                                                from: Offset,
                                                to: Offset)
                                                -> Result<(), Box<Error>> {
        let arr = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<Hash> = Vec::new();
        for el in arr {
            let stri = el.as_str().ok_or("Can't cast json as string")?;
            let hash = <Hash as HexValue>::from_hex(stri)?;
            vec.push(hash)
        }
        buffer.write(from, to, vec.as_slice());
        Ok(())

    }
}
impl<'a> ExonumJsonDeserializeField for &'a [u8] {
    fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                buffer: &mut B,
                                                from: Offset,
                                                to: Offset)
                                                -> Result<(), Box<Error>> {
        let bytes = value.as_str().ok_or("Can't cast json as string")?;
        let arr = <Vec<u8> as HexValue>::from_hex(bytes)?;
        buffer.write(from, to, arr.as_slice());
        Ok(())
    }
}

impl ExonumJsonDeserializeField for Vec<Arc<::messages::MessageBuffer>> {
    fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                buffer: &mut B,
                                                from: Offset,
                                                to: Offset)
                                                -> Result<(), Box<Error>> {
        use messages::MessageBuffer;
        let bytes = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<_> = Vec::new();
        for el in bytes {
            let stri = el.as_str().ok_or("Can't cast json as string")?;
            let str_hex = <Vec<u8> as HexValue>::from_hex(stri)?;
            vec.push(Arc::new(MessageBuffer::from_vec(str_hex)));
        }
        buffer.write(from, to, vec);
        Ok(())
    }
}


impl<T> ExonumJsonDeserialize for Vec<T>
    where T: ExonumJsonDeserialize,
          for<'a> Vec<T>: Field<'a>
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

impl<T> ExonumJsonDeserializeField for Vec<T>
    where T: ExonumJsonDeserialize,
          for<'a> Vec<T>: Field<'a>
{
    fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                buffer: &mut B,
                                                from: Offset,
                                                to: Offset)
                                                -> Result<(), Box<Error>> {
        let bytes = value.as_array().ok_or("Can't cast json as array")?;
        let mut vec: Vec<_> = Vec::new();
        for el in bytes {
            let obj = T::deserialize(el)?;
            vec.push(obj);
        }
        buffer.write(from, to, vec);
        Ok(())
    }
}

impl ExonumJsonDeserializeField for BitVec {
    fn deserialize_field<B: WriteBufferWrapper>(value: &Value,
                                                buffer: &mut B,
                                                from: Offset,
                                                to: Offset)
                                                -> Result<(), Box<Error>> {
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
}


/// Reexport of `serde` specific traits, this reexports
/// provide compatibility layer with important `serde_json` version.
pub mod reexport {
    pub use serde_json::{Value, to_value, from_value, to_string, from_str};
}
