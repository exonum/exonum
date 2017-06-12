//! Serialize structure into specific format.
//! Currently support only json.
//! This module is a pack of superstructures over serde `Serializer's`\\`Deserializer's`

pub use hex::{FromHexError, ToHex, FromHex};
use encoding::Field;
use messages::MessageWriter;
use super::Offset;

#[macro_export]
/// Calculate num of idents in macro call.
/// Used by `message!` and `encoding_struct!`
macro_rules! idents_count {
    () => (0usize);
    ($head:ident $($tail:ident)*) => (1usize + idents_count!($($tail)*))
}

/// implement exonum serialization\deserialization based on serde `Serialize`\ `Deserialize`
///
/// Item should implement:
///
/// - `serde::Serialize`
/// - `serde::Deserialize`
/// - `exonum::encoding::Field`
///
/// **Beware, this macros probably implement traits in not optimal way.**
#[macro_export]
macro_rules! implement_exonum_serializer {
    ($name:ident) => {
        impl $crate::encoding::serialize::json::ExonumJsonDeserialize for $name {
            fn deserialize(value: &$crate::encoding::serialize::json::reexport::Value)
                                                        -> Result<$name, Box<::std::error::Error>> {
                use $crate::encoding::serialize::json::reexport::from_value;
                Ok(from_value(value.clone())?)
            }
        }

        impl $crate::encoding::serialize::json::ExonumJson for $name {
            fn deserialize_field<B>(
                value: &$crate::encoding::serialize::json::reexport::Value,
                                                        buffer: &mut B,
                                                        from: $crate::encoding::Offset,
                                                        to: $crate::encoding::Offset)
                                                        -> Result<(), Box<::std::error::Error>> 
            where B: $crate::encoding::serialize::WriteBufferWrapper
            {
                use $crate::encoding::serialize::json::reexport::from_value;
                let value: $name = from_value(value.clone())?;
                buffer.write(from, to, value);
                Ok(())
            }

            fn serialize_field(&self) -> Result<Value, Box<Error>> {
                use $crate::encoding::serialize::json::reexport::to_value;
                Ok(to_value(self)?)
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

/// Reexport of `serde` specific traits, this reexports
/// provide compatibility layer with important `serde` version.
pub mod reexport {
    pub use serde::{Serializer, Deserializer, Serialize, Deserialize};
    pub use serde::de::Error as DeError;
    pub use serde::ser::Error as SerError;
    pub use serde::ser::SerializeStruct;
}
