//! `stream_struct` is a lazy serialization library,
//! it allows to keep struct serialized in place, and deserialize fields on demand.
//!
//! Binary representation structure splitted into two main parts:
//!
//! - Header - fixed sized part.
//! - Body - dynamic sized, known only after parsing header, part.
//!
//! For easy creating this structures,
//! in exonum you can use macros `message!` and `storage_value!`
//! #Examples
//! Imagine structure with just two field's
//!
//! - First one is `String`
//! - second one is `u64`
//!
//! To create message for this structure now, you need to know how many bytes this fields
//! took in header. See [Field layout].
//!
//! We know that `u64` [took 8 bytes](#primitive-types),
//! and string took [8 segment bytes](#segment-fields).
//!
//! Then to create, for example [storage value](../macro.storage_value.html),
//! you need to use macros like this:
//!
//! ```
//! # #[macro_use] extern crate exonum;
//! # extern crate serde;
//! # extern crate serde_json;
//! storage_value! {
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
//! # }
//! ```
//! Then in memory of student you will get:
//!
//! | Position | Stored data  | Hexadecimal form | Comment |
//! |:--------|:------:|:---------------------|:--------------------------------------------------|
//! `0  => 4`  | 16    | `10 00 00 00`            | Little endian storred segment pointer, reffer to position in data where real string is located |
//! `4  => 8`  | 6     | `06 00 00 00`            | Little endian storred segment size |
//! `8  => 16` | 23    | `10 00 00 00 00 00 00 00`| number in little endian |
//! `16 => 24` | Andrew| `41 6e 64 72 65 77`	    | Real text bytes|
//!
//!
//! #Fields layout
//! Fields could be splitted into tree main parts:
//!
//! ### Primitive types
//! 
//! Primitive types are all fixed sized, and located fully in header.
//\TODO explain how an integer is stored in memory (what codding)
//!
//! | Type name | Size in Header | Info |
//! |:--------|:---------------------|:--------------------------------------------------|
//! `u8`     | 1    | Regular byte  |
//! `i8`     | 1    | Signed byte  |
//! `u16`    | 2    | Short unsigned number storred in little endian  |
//! `i16`    | 2    | Short signed number storred in little endian  |
//! `u32`    | 4    | 32-bit unsigned number storred in little endian  |
//! `i32`    | 4    | 32-bit signed number storred in little endian  |
//! `u64`    | 8    | long unsigned number storred in little endian  |
//! `i64`    | 8    | long signed number storred in little endian  |
//! `bool`   | 1    | stored as single byte, where `0x01` - true `0x00` - false [\[1\]](#1)|
//! 
//! ######\[1] 
//! **Trying to represent other values as bool lead to undefined behavior**.
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
//! but they should to declare how many bytes they 
//! will write on header [in function `field_size()`](./trait.Field.html#tymethod.field_size)
//! 

pub use self::fields::Field;
pub use self::error::Error;

#[macro_use]
pub mod serialize;

mod error;
mod fields;
mod segments;

#[cfg(test)]
mod tests;

type Offset = u32;

pub type Result = ::std::result::Result<Option<SegmentReference>, Error>;

pub struct SegmentReference {
    pub from: Offset,
    pub to: Offset,
}

impl SegmentReference {
    pub fn new(from: Offset, to: Offset) -> SegmentReference {
        SegmentReference { from: from, to: to }
    }

    pub fn check_segment(&mut self,
                         header_size: u32,
                         last_data: &mut u32)
                         -> ::std::result::Result<(), Error> {
        if self.from < header_size {
            Err(Error::SementInHeader {
                    header_size: header_size,
                    start: self.from,
                })
        } else if self.from < *last_data {
            Err(Error::OverlappingSegment {
                    last_end: *last_data,
                    start: self.from,
                })
        } else if self.from > *last_data {
            Err(Error::SpaceBetweenSegments {
                    last_end: *last_data,
                    start: self.from,
                })
        } else {
            *last_data = self.to;
            Ok(())
        }
    }
}
