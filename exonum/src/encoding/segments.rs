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

use byteorder::{ByteOrder, LittleEndian};
use bit_vec::BitVec;

use messages::{RawMessage, HEADER_LENGTH, MessageBuffer};
use crypto::Hash;

use super::{Result, Error, Field, Offset, CheckedOffset};

/// Trait for fields, that has unknown `compile-time` size.
/// Usually important for arrays,
/// or other types that in rust is always at `HEAP`
pub trait SegmentField<'a>: Sized {
    /// size of item fixed part that this `Field` collect.
    fn item_size() -> Offset;
    /// count of items in collection
    fn count(&self) -> Offset;
    /// create collection from buffer
    unsafe fn from_buffer(buffer: &'a [u8], from: Offset, count: Offset) -> Self;
    /// extend buffer with this collection
    fn extend_buffer(&self, buffer: &mut Vec<u8>);

    #[allow(unused_variables)]
    /// check collection data
    fn check_data(
        buffer: &'a [u8],
        from: CheckedOffset,
        count: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result;
}

impl<'a, T> Field<'a> for T
where
    T: SegmentField<'a>,
{
    fn field_size() -> Offset {
        8
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> T {
        let pos = LittleEndian::read_u32(&buffer[from as usize..from as usize + 4]);
        let count = LittleEndian::read_u32(&buffer[from as usize + 4..to as usize]);
        Self::from_buffer(buffer, pos, count)
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        let pos = buffer.len() as u32;
        LittleEndian::write_u32(&mut buffer[from as usize..from as usize + 4], pos);
        LittleEndian::write_u32(
            &mut buffer[from as usize + 4..to as usize],
            self.count() as u32,
        );
        self.extend_buffer(buffer);

    }

    fn check(
        buffer: &'a [u8],
        pointer_from: CheckedOffset,
        pointer_to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        debug_assert_eq!(
            (pointer_to - pointer_from)?.unchecked_offset(),
            Self::field_size()
        );
        let pointer_count_start: Offset = (pointer_from + 4)?.unchecked_offset();
        let segment_start: CheckedOffset = LittleEndian::read_u32(
            &buffer[pointer_from.unchecked_offset() as usize..
                        pointer_count_start as usize],
        ).into();
        let count: CheckedOffset = LittleEndian::read_u32(
            &buffer[pointer_count_start as usize..
                        pointer_to.unchecked_offset() as usize],
        ).into();

        if segment_start < latest_segment {
            return Err(Error::OverlappingSegment {
                last_end: latest_segment.unchecked_offset(),
                start: segment_start.unchecked_offset(),
            });
        } else if segment_start > latest_segment {
            return Err(Error::SpaceBetweenSegments {
                last_end: latest_segment.unchecked_offset(),
                start: segment_start.unchecked_offset(),
            });
        }

        let segment_end = (segment_start + (count * Self::item_size())?)?;
        if segment_end.unchecked_offset() > buffer.len() as u32 {
            return Err(Error::IncorrectSegmentSize {
                position: pointer_count_start,
                value: count.unchecked_offset(),
            });
        }

        let latest_segment = segment_end;

        Self::check_data(buffer, segment_start, count, latest_segment)
    }
}

impl<'a> SegmentField<'a> for &'a str {
    fn item_size() -> Offset {
        1
    }

    fn count(&self) -> Offset {
        self.as_bytes().len() as Offset
    }

    unsafe fn from_buffer(buffer: &'a [u8], from: Offset, count: Offset) -> Self {
        let to = from + count * Self::item_size();
        let slice = &buffer[from as usize..to as usize];
        ::std::str::from_utf8_unchecked(slice)
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self.as_bytes())
    }

    fn check_data(
        buffer: &'a [u8],
        from: CheckedOffset,
        count: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        let size: CheckedOffset = (count * Self::item_size())?;
        let to: CheckedOffset = (from + size)?;
        let slice = &buffer[from.unchecked_offset() as usize..to.unchecked_offset() as usize];
        if let Err(e) = ::std::str::from_utf8(slice) {
            return Err(Error::Utf8 {
                position: from.unchecked_offset(),
                error: e,
            });
        }
        Ok(latest_segment)
    }
}

impl<'a> SegmentField<'a> for RawMessage {
    fn item_size() -> Offset {
        1
    }

    fn count(&self) -> Offset {
        self.as_ref().as_ref().len() as Offset
    }

    unsafe fn from_buffer(buffer: &'a [u8], from: Offset, to: Offset) -> Self {
        let to = from + to * Self::item_size();
        let slice = &buffer[from as usize..to as usize];
        RawMessage::new(MessageBuffer::from_vec(Vec::from(slice)))
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {

        buffer.extend_from_slice(self.as_ref().as_ref())
    }

    fn check_data(
        buffer: &'a [u8],
        from: CheckedOffset,
        count: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        let size: CheckedOffset = (count * Self::item_size())?;
        let to: CheckedOffset = (from + size)?;
        let slice = &buffer[from.unchecked_offset() as usize..to.unchecked_offset() as usize];
        if slice.len() < HEADER_LENGTH {
            return Err(Error::UnexpectedlyShortRawMessage {
                position: from.unchecked_offset(),
                size: slice.len() as Offset,
            });
        }
        let actual_size = slice.len() as Offset;
        let declared_size: Offset = LittleEndian::read_u32(&slice[6..10]);
        if actual_size != declared_size {
            return Err(Error::IncorrectSizeOfRawMessage {
                position: from.unchecked_offset(),
                actual_size: slice.len() as Offset,
                declared_size: declared_size,
            });
        }
        Ok(latest_segment)
    }
}

impl<'a, T> SegmentField<'a> for Vec<T>
where
    T: Field<'a>,
{
    fn item_size() -> Offset {
        T::field_size()
    }

    fn count(&self) -> Offset {
        self.len() as Offset
    }

    // TODO: implement different
    // for Vec<T> where T: Field,
    // for Vec<T> where T = u8
    // but this is possible only after specialization land (ECR-156)
    unsafe fn from_buffer(buffer: &'a [u8], from: Offset, count: Offset) -> Self {
        // read vector len
        let mut vec = Vec::with_capacity(count as usize);
        let mut start = from;
        for _ in 0..count {
            vec.push(T::read(buffer, start, start + Self::item_size()));
            start += Self::item_size();
        }
        vec
    }

    fn extend_buffer(&self, mut buffer: &mut Vec<u8>) {
        let mut start = buffer.len() as Offset;
        buffer.resize((start + self.count() * Self::item_size()) as usize, 0);
        // write rest of fields
        for i in self.iter() {
            i.write(&mut buffer, start, start + Self::item_size());
            start += Self::item_size();
        }
    }

    fn check_data(
        buffer: &'a [u8],
        from: CheckedOffset,
        count: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        let mut start = from;
        let mut latest_segment = latest_segment;

        for _ in 0..count.unchecked_offset() {
            latest_segment = T::check(buffer, start, (start + Self::item_size())?, latest_segment)?;
            start = (start + Self::item_size())?;
        }
        Ok(latest_segment)
    }
}

impl<'a> SegmentField<'a> for BitVec {
    fn item_size() -> Offset {
        1
    }

    // TODO: reduce memory allocation (ECR-156)
    fn count(&self) -> Offset {
        self.to_bytes().len() as Offset
    }

    unsafe fn from_buffer(buffer: &'a [u8], from: Offset, count: Offset) -> Self {
        let to = from + count * Self::item_size();
        let slice = &buffer[from as usize..to as usize];
        BitVec::from_bytes(slice)
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        // TODO: avoid reallocation here using normal implementation of bitvec (ECR-156)
        let slice = &self.to_bytes();
        buffer.extend_from_slice(slice);
    }

    fn check_data(
        _: &'a [u8],
        _: CheckedOffset,
        _: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        Ok(latest_segment)
    }
}

impl<'a> SegmentField<'a> for &'a [u8] {
    fn item_size() -> Offset {
        1
    }

    fn count(&self) -> Offset {
        self.len() as Offset
    }

    unsafe fn from_buffer(buffer: &'a [u8], from: Offset, count: Offset) -> Self {
        let to = from + count * Self::item_size();
        &buffer[from as usize..to as usize]
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self)
    }

    fn check_data(
        _: &'a [u8],
        _: CheckedOffset,
        _: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        Ok(latest_segment)
    }
}


/// Implement field helper for all array of POD types
/// it writes POD type as bytearray in place.
///
/// **Beware of platform specific data representation.**
#[macro_export]
macro_rules! implement_pod_array_field {
    ($name:ident) => (

        impl<'a> SegmentField<'a> for &'a [$name] {
            fn item_size() -> Offset {
                ::std::mem::size_of::<$name>() as Offset
            }

            fn count(&self) -> Offset {
                self.len() as Offset
            }

            unsafe fn from_buffer(buffer: &'a [u8], from: Offset, count: Offset) -> Self {
                let to = from + count * Self::item_size();
                let slice = &buffer[(from as usize)..(to as usize)];
                ::std::slice::from_raw_parts(slice.as_ptr() as *const Hash,
                                            slice.len() / Self::item_size() as usize)
            }

            fn extend_buffer(&self, buffer: &mut Vec<u8>) {
                let slice = unsafe {
                    ::std::slice::from_raw_parts(self.as_ptr() as *const u8,
                                                self.len() * Self::item_size() as usize)
                };
                buffer.extend_from_slice(slice)
            }

            fn check_data(_: &'a [u8],
                        _: CheckedOffset,
                        _: CheckedOffset,
                        latest_segment: CheckedOffset) -> Result {
                Ok(latest_segment)
            }
        }
    )
}

implement_pod_array_field!{Hash}
