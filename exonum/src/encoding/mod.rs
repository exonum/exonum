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

//! `encoding` is a serialization library supporting zero-copy (de)serialization
//! of primitive types, heterogeneous structures and arrays.
//!
//! See also [the documentation page on serialization][doc:serialization].
//!
//! # Structure serialization
//!
//! Structures are in the root of any serializable Exonum object.
//! Binary representation of structures is split into two main parts:
//!
//! - **Header:** a fixed-sized part
//! - **Body:** dynamically sized part, known only after parsing the header
//!
//! To create a structure type, you can use [`transactions!`] and [`encoding_struct!`] macros.
//!
//! [doc:serialization]: https://exonum.com/doc/architecture/serialization/
//! [`transactions!`]: ../macro.transactions.html
//! [`encoding_struct!`]: ../macro.encoding_struct.html
//!
//! # Examples
//!
//! Consider a structure with two fields: `String` and `u64`.
//! To implement Exonum (de)serialization for this structure
//! you need to use macros like this:
//!
//! ```
//! # #[macro_use] extern crate exonum;
//! # extern crate serde;
//! # extern crate serde_json;
//! encoding_struct! {
//!     struct MyAwesomeStructure {
//!         name: &str,
//!         age: u64,
//!     }
//! }
//!
//! # fn main() {
//! let student = MyAwesomeStructure::new("Andrew", 23);
//! # }
//! ```
//!
//! Then the internal buffer of `student` is as follows:
//!
//! | Position | Stored data | Hexadecimal form | Comment |
//! |--------|------|---------------------|------------------------------------------|
//! | `0  => 4`  | 16    | `10 00 00 00`            | LE-encoded segment pointer to the data |
//! | `4  => 8`  | 6     | `06 00 00 00`            | LE-encoded segment size |
//! | `8  => 16` | 23    | `17 00 00 00 00 00 00 00` | number in little endian |
//! | `16 => 24` | Andrew | `41 6e 64 72 65 77` | Text bytes in UTF-8 encoding |
//!
//! # Structure fields
//!
//! ## Primitive types
//!
//! Primitive types are all fixed-sized, and located fully in the header.
//!
//! | Type name | Size in Header | Info |
//! |--------|---------------------|--------------------------------------------------|
//! | `u8`     | 1    | Regular byte  |
//! | `i8`     | 1    | Signed byte  |
//! | `u16`    | 2    | Short unsigned integer stored in little endian  |
//! | `i16`    | 2    | Short signed integer stored in little endian  |
//! | `u32`    | 4    | 32-bit unsigned integer stored in little endian  |
//! | `i32`    | 4    | 32-bit signed integer stored in little endian  |
//! | `u64`    | 8    | Long unsigned integer stored in little endian  |
//! | `i64`    | 8    | Long signed integer stored in little endian  |
//! | `F32`    | 4    | 32-bit floating point type stored in little endian \[1\]\[2\] |
//! | `F64`    | 8    | 64-bit floating point type stored in little endian \[1\]\[2\] |
//! | `bool`   | 1    | Stored as a byte, with `0x01` denoting true and `0x00` false \[3\] |
//!
//! \[1\]
//! Special floating point values that cannot be represented as a sequences of digits (such as
//! Infinity, NaN and signaling NaN) are not permitted.
//!
//! \[2\]
//! Floating point value serialization is hidden behind the `float_serialize` feature gate.
//!
//! \[3\]
//! Trying to represent other values as `bool` leads to undefined behavior.
//!
//! ## Segment fields
//!
//! All segment types take 8 bytes in the header: 4 for position in the buffer,
//! and 4 for the segment field size.
//!
//! ## Custom fields
//!
//! These types can be implemented as per developer's design,
//! but they should declare how many bytes they
//! write in the header using the [`field_size()`] function.
//!
//! [`field_size()`]: ./trait.Field.html#tymethod.field_size

#[cfg(feature = "float_serialize")]
pub use self::float::{F32, F64};
pub use self::{error::Error, fields::Field, segments::SegmentField};

#[macro_use]
pub mod serialize;

use std::{
    convert::From, ops::{Add, Div, Mul, Sub},
};

mod error;
#[macro_use]
mod fields;
mod segments;
#[macro_use]
mod spec;
#[cfg(feature = "float_serialize")]
mod float;

#[cfg(test)]
mod tests;

/// Type alias usable for reference in buffer
pub type Offset = u32;

/// Type alias that should be returned in `check` method of `Field`
pub type Result = ::std::result::Result<CheckedOffset, Error>;

// TODO: Replace by more generic type. (ECR-156)
/// `CheckedOffset` is a type that take control over overflow,
/// so you can't panic without `unwrap`,
/// and work with this value without overflow checks.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct CheckedOffset {
    offset: Offset,
}

impl CheckedOffset {
    /// create checked value
    pub fn new(offset: Offset) -> CheckedOffset {
        CheckedOffset { offset }
    }

    /// return unchecked offset
    pub fn unchecked_offset(self) -> Offset {
        self.offset
    }
}

macro_rules! implement_default_ops_checked {
    ($trait_name:ident $function:ident $checked_function:ident) => {
        impl $trait_name<CheckedOffset> for CheckedOffset {
            type Output = ::std::result::Result<CheckedOffset, Error>;
            fn $function(self, rhs: CheckedOffset) -> Self::Output {
                self.offset
                    .$checked_function(rhs.offset)
                    .map(CheckedOffset::new)
                    .ok_or(Error::OffsetOverflow)
            }
        }
        impl $trait_name<Offset> for CheckedOffset {
            type Output = ::std::result::Result<CheckedOffset, Error>;
            fn $function(self, rhs: Offset) -> Self::Output {
                self.offset
                    .$checked_function(rhs)
                    .map(CheckedOffset::new)
                    .ok_or(Error::OffsetOverflow)
            }
        }
    };
}

implement_default_ops_checked!{Add add checked_add }
implement_default_ops_checked!{Sub sub checked_sub }
implement_default_ops_checked!{Mul mul checked_mul }
implement_default_ops_checked!{Div div checked_div }

impl From<Offset> for CheckedOffset {
    fn from(offset: Offset) -> CheckedOffset {
        CheckedOffset::new(offset)
    }
}
