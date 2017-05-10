///! stream_struct is a lazy serialization library, 
///! it allows to keep struct serialized in place, and deserialize fields on demand.

pub use self::fields::Field;
pub use self::error::Error;

#[macro_use]
pub mod serialize;

mod error;
mod fields;
mod segments;

#[cfg(test)]
mod tests;