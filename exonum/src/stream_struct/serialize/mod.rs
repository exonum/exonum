//! Serialize structure into specific format.
//! Currently support only json.
//! This module is a pack of superstructures over serde `Serializer's`\\`Deserializer's`

pub use hex::{FromHexError, ToHex, FromHex};
use stream_struct::Field;
use messages::MessageWriter;
use super::Offset;
// for all internal serializers, implement default realization
macro_rules! impl_default_serialize {
    (@impl $traitname:ty; $typename:ty) => {
        impl $traitname for $typename {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: $crate::stream_struct::serialize::reexport::Serializer
            {
                <Self as ::serde::Serialize>::serialize(self, serializer)
            }
        }
    };
    ($traitname:ty => $($name:ty);*) => ($(impl_default_serialize!{@impl $traitname; $name})*);
}

// for all internal serializers, implement default realization-deref
macro_rules! impl_default_serialize_deref {
    (@impl $traitname:ident $typename:ty) => {
        impl<'a> $traitname for &'a $typename {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: $crate::stream_struct::serialize::reexport::Serializer
            {
                <$typename as ::serde::Serialize>::serialize(*self, serializer)
            }
        }
    };
    ($traitname:ident => $($name:ty);*) =>
        ($(impl_default_serialize_deref!{@impl $traitname $name})*);
}

/// implement exonum serialization\deserialization based on serde `Serialize`\ `Deserialize`
///
/// Item should implement:
///
/// - `serde::Serialize`
/// - `serde::Deserialize`
/// - `exonum::stream_struct::Field`
///
/// **Beware, this macros probably implement traits in not optimal way.**
#[macro_export]
macro_rules! implement_exonum_serializer {
    ($name:ident) => {
        impl $crate::stream_struct::serialize::json::ExonumJsonSerialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: $crate::stream_struct::serialize::reexport::Serializer
            {
                <$name as ::serde::Serialize>::serialize(self, serializer)
            }
        }

        impl $crate::stream_struct::serialize::json::ExonumJsonDeserialize for $name {
            fn deserialize(value: &$crate::stream_struct::serialize::json::reexport::Value)
                                                        -> Result<$name, Box<::std::error::Error>> {
                use $crate::stream_struct::serialize::json::reexport::from_value;
                Ok(from_value(value.clone())?)
            }
        }

        impl $crate::stream_struct::serialize::json::ExonumJsonDeserializeField for $name {
            fn deserialize_field<B>(
                value: &$crate::stream_struct::serialize::json::reexport::Value,
                                                        buffer: &mut B,
                                                        from: $crate::stream_struct::Offset,
                                                        to: $crate::stream_struct::Offset)
                                                        -> Result<(), Box<::std::error::Error>> 
            where B: $crate::stream_struct::serialize::WriteBufferWrapper
            {
                use $crate::stream_struct::serialize::json::reexport::from_value;
                let value: $name = from_value(value.clone())?;
                buffer.write(from, to, value);
                Ok(())
            }
        }

    };
}


/// implement serializing wrappers and methods for json
#[macro_use]
pub mod json;

/// `HexValue` is a converting trait,
/// for values that could be converted from hex `String`,
/// and writed as hex `String`
pub trait HexValue: Sized {
    /// Format value as hex representation.
    fn to_hex(&self) -> String;
    /// Convert value from hex representation.
    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError>;
}

/// `WriteBufferWrapper` is a trait specific for writing fields in place.
#[doc(hidden)]
pub trait WriteBufferWrapper {
    fn write<'a, T: Field<'a>>(&'a mut self, from: Offset, to: Offset, val: T);
}

impl WriteBufferWrapper for MessageWriter {
    fn write<'a, T: Field<'a>>(&'a mut self, from: Offset, to: Offset, val: T) {
        self.write(val, from, to)
    }
}

impl WriteBufferWrapper for Vec<u8> {
    fn write<'a, T: Field<'a>>(&'a mut self, from: Offset, to: Offset, val: T) {
        val.write(self, from, to)
    }
}

#[macro_use]
mod utils;


/// Reexport of `serde` specific traits, this reexports
/// provide compatibility layer with important `serde` version.
pub mod reexport {
    pub use serde::{Serializer, Deserializer, Serialize, Deserialize};
    pub use serde::de::Error;
    pub use serde::ser::SerializeStruct;
}
