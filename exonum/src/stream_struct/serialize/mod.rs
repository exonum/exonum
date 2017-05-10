pub use hex::{FromHexError, ToHex, FromHex};
// for all internal serializers, implement default realization
macro_rules! impl_default_serialize {
    (@impl $traitname:ident $typename:ty) => {
        impl $traitname for $typename {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                <Self as ::serde::Serialize>::serialize(self, serializer)
            }
        }
    };
    ($traitname:ident => $($name:ty);*) => ($(impl_default_serialize!{@impl $traitname $name})*);
}

// for all internal serializers, implement default realization-deref
macro_rules! impl_default_serialize_deref {
    (@impl $traitname:ident $typename:ty) => {
        impl<'a> $traitname for &'a $typename {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                <$typename as ::serde::Serialize>::serialize(*self, serializer)
            }
        }
    };
    ($traitname:ident => $($name:ty);*) => ($(impl_default_serialize_deref!{@impl $traitname $name})*);
}


/// implement serializing wrappers and methods for json
#[macro_use]
pub mod json;

pub trait HexValue: Sized {
    fn to_hex(&self) -> String;
    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError>;
}

#[macro_use]
mod utils;
