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

//! `encoding` is a lazy serialization library,
//! it allows to keep struct serialized in place, and deserialize fields on demand.
//!
//! Binary representation of structure is split into two main parts:
//!
//! - Header - fixed sized part.
//! - Body - dynamic sized, known only after parsing header, part.
//!
//! For easy creating this structures,
//! in exonum you can use macros `message!` and `encoding_struct!`
//! #Examples
//! Imagine structure with just two field's
//!
//! - First one is `String`
//! - second one is `u64`
//!
//! To create message for this structure now, you need to know how many bytes this fields
//! took in header. See [Fields layout](#fields-layout).
//!
//! We know that `u64` [took 8 bytes](#primitive-types),
//! and string took [8 segment bytes](#segment-fields).
//!
//! Then to create, for example [storage value](../macro.encoding_struct.html),
//! you need to use macros like this:
//!
//! ```
//! # #[macro_use] extern crate exonum;
//! # extern crate serde;
//! # extern crate serde_json;
//! encoding_struct! {
//!     struct MyAwesomeStructure {
//!         const SIZE = 16;
//!
//!         field name: &str [0 => 8]
//!         field age:  u64  [8 => 16]
//!     }
//! }
//! // now if we create it in memory
//!
//! # fn main() {
//!     let student = MyAwesomeStructure::new("Andrew", 23);
//! # drop(student);
//! # }
//! ```
//! Then in internal buffer of `student` you will get:
//!
//! | Position | Stored data  | Hexadecimal form | Comment |
//! |:--------|:------:|:---------------------|:--------------------------------------------------|
//! `0  => 4`  | 16    | `10 00 00 00`            | LE stored segment pointer to the data |
//! `4  => 8`  | 6     | `06 00 00 00`            | LE stored segment size |
//! `8  => 16` | 23    | `17 00 00 00 00 00 00 00`| number in little endian |
//! `16 => 24` | Andrew| `41 6e 64 72 65 77`	    | Real text bytes|
//!
//!
//! #Fields layout
//! Fields could be splitted into tree main parts:
//!
//! ### Primitive types
//!
//! Primitive types are all fixed sized, and located fully in header.
// TODO explain how an signed integer is stored in memory (what codding) (ECR-155)
//!
//! | Type name | Size in Header | Info |
//! |:--------|:---------------------|:--------------------------------------------------|
//! `u8`     | 1    | Regular byte  |
//! `i8`     | 1    | Signed byte  |
//! `u16`    | 2    | Short unsigned number stored in little endian  |
//! `i16`    | 2    | Short signed number stored in little endian  |
//! `u32`    | 4    | 32-bit unsigned number stored in little endian  |
//! `i32`    | 4    | 32-bit signed number stored in little endian  |
//! `u64`    | 8    | long unsigned number stored in little endian  |
//! `i64`    | 8    | long signed number stored in little endian  |
//! `bool`   | 1    | stored as single byte, where `0x01` - true `0x00` - false [\[1\]](#1)|
//!
//! ######\[1]
//! **Trying to represent other values as bool leads to undefined behavior**.
//!
//! ### Segment fields
//!
//! All segment types took 8 bytes in header,
//! 4 for position in buffer,
//! and 4 for segment field size
//!
//! ### Custom fields
//!
//! This types could be implemented as creator want,
//! but they should declare how many bytes they
//! will write on header [in function `field_size()`](./trait.Field.html#tymethod.field_size)
//!

use std::convert::From;
use std::ops::{Add, Sub, Mul, Div};

pub use self::fields::Field;
pub use self::segments::SegmentField;
pub use self::error::Error;

#[macro_use]
pub mod serialize;

mod error;
#[macro_use]
mod fields;
mod segments;
#[macro_use]
mod spec;


#[cfg(test)]
mod tests;

/// Type alias usable for reference in buffer
pub type Offset = u32;

/// Type alias that should be returned in `check` method of `Field`
pub type Result = ::std::result::Result<CheckedOffset, Error>;

// TODO replace by more generic type (ECR-156).
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
        CheckedOffset { offset: offset }
    }

    /// return unchecked offset
    pub fn unchecked_offset(self) -> Offset {
        self.offset
    }
}

macro_rules! implement_default_ops_checked {
    ($trait_name: ident $function:ident $checked_function:ident) => (
        impl $trait_name<CheckedOffset> for CheckedOffset {
            type Output = ::std::result::Result<CheckedOffset, Error>;
            fn $function(self, rhs: CheckedOffset) -> Self::Output {
                self.offset.$checked_function(rhs.offset)
                        .map(CheckedOffset::new)
                        .ok_or(Error::OffsetOverflow)
            }
        }
        impl $trait_name<Offset> for CheckedOffset {
            type Output = ::std::result::Result<CheckedOffset, Error>;
            fn $function(self, rhs: Offset) -> Self::Output {
                self.offset.$checked_function(rhs)
                        .map(CheckedOffset::new)
                        .ok_or(Error::OffsetOverflow)
            }
        }
    )
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
