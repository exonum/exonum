///! stream_struct is a lazy serialization library, 
///! it allows to keep struct serialized in place, and deserialize fields on demand.
///! For serialization purposes into binary representation structure splitted into two main parts:
///!
///! Header - fixed sized part.
///! And Body - dynamic sized, known only after parsing header, part.
///! 

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
    pub size: Offset
}

impl SegmentReference {
    pub fn new(from: Offset, size: Offset) -> SegmentReference {
        SegmentReference {
            from: from,
            size: size
        }
    }

    pub fn check_segment(&mut self, header_size: u32, last_data: &mut u32)
        -> ::std::result::Result<(), Error>
    {
        if self.from < header_size {
            Err(Error::SementInHeader{header_size: header_size, start: self.from})
        }
        else if self.from < *last_data {
            Err(Error::OverlappingSegment{last_end: *last_data, start: self.from})
        }
        else if self.from > *last_data {
            Err(Error::SpaceBetweenSegments{last_end: *last_data, start: self.from})
        } else {
            *last_data = self.from + self.size;
            Ok(())
        }
    }
}

