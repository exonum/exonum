use byteorder::{ByteOrder, LittleEndian};

use messages::{BitVec, RawMessage, HEADER_SIZE, MessageBuffer};
use crypto::Hash;

use super::{Result, Error, Field, SegmentReference, Offset, CheckedOffset};

pub trait SegmentField<'a>: Sized {
    fn item_size() -> Offset;
    fn count(&self) -> Offset;
    unsafe fn from_buffer(buffer: &'a [u8], from: Offset, count: Offset) -> Self;
    fn extend_buffer(&self, buffer: &mut Vec<u8>);

    #[allow(unused_variables)]
    fn check_data(buffer: &'a [u8], from: CheckedOffset, count: CheckedOffset) -> Result {
        let size: CheckedOffset = (count * Self::item_size())?;
        let to: CheckedOffset = (from + size)?;
        Ok(Some(SegmentReference::new(from, to)))
    }
}

impl<'a, T> Field<'a> for T
    where T: SegmentField<'a>
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
        LittleEndian::write_u32(&mut buffer[from as usize + 4..to as usize],
                                self.count() as u32);
        self.extend_buffer(buffer);

    }

    fn check(buffer: &'a [u8],
             header_from: CheckedOffset,
             header_to: CheckedOffset) -> Result {
        check_field_size!{buffer header_from; header_to};
        let header_count_start: Offset = (header_from + 4)?.unchecked_offset();
        let segment_start: CheckedOffset = LittleEndian::read_u32(
                                &buffer[header_from.unchecked_offset() as usize
                                            ..header_count_start as usize])
                .into();
        let count: CheckedOffset = LittleEndian::read_u32(
                                &buffer[header_count_start as usize
                                            ..header_to.unchecked_offset() as usize])
                .into();

        if count.unchecked_offset() == 0 {
            return Ok(None);
        }

        if segment_start < header_to {
            return Err(Error::IncorrectSegmentReference {
                           position: header_from.unchecked_offset(),
                           value: segment_start.unchecked_offset(),
                       });
        }

        let segment_end = (segment_start + (count * Self::item_size())?)?;
        if segment_end.unchecked_offset() > buffer.len() as u32 {
            return Err(Error::IncorrectSegmentSize {
                           position: header_count_start,
                           value: count.unchecked_offset(),
                       });
        }

        Self::check_data(buffer, segment_start, count)
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

    fn check_data(buffer: &'a [u8], from: CheckedOffset, count: CheckedOffset) -> Result {
        let size: CheckedOffset = (count * Self::item_size())?;
        let to: CheckedOffset = (from + size)?;
        let slice = &buffer[from.unchecked_offset() as usize..to.unchecked_offset() as usize];
        if let Err(e) = ::std::str::from_utf8(slice) {
            return Err(Error::Utf8 {
                           position: from.unchecked_offset(),
                           error: e,
                       });
        }
        Ok(Some(SegmentReference::new(from, to)))
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

    fn check_data(buffer: &'a [u8], from: CheckedOffset, count: CheckedOffset) -> Result {
        let size: CheckedOffset = (count * Self::item_size())?;
        let to: CheckedOffset = (from + size)?;
        let slice = &buffer[from.unchecked_offset() as usize..to.unchecked_offset() as usize];
        if slice.len() < HEADER_SIZE {
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
        Ok(Some(SegmentReference::new(from, to)))
    }
}

impl<'a, T> SegmentField<'a> for Vec<T>
    where T: Field<'a>
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
    // but this is possible only after specialization land
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

    fn check_data(buffer: &'a [u8], from: CheckedOffset, count: CheckedOffset) -> Result {
        let mut start = from;
        let size: CheckedOffset = (count * Self::item_size())?;
        let header = (start + size)?;
        let mut last_data = header;

        for _ in 0..count.unchecked_offset() {
            T::check(buffer, start, (start + Self::item_size())?)?
                .map_or(Ok(()), |mut e| e.check_segment(&mut last_data))?;
            start = (start + Self::item_size())?;
        }
        Ok(Some(SegmentReference::new(from, last_data)))
    }
}

impl<'a> SegmentField<'a> for BitVec {
    fn item_size() -> Offset {
        1
    }

    // TODO: reduce memory allocation
    fn count(&self) -> Offset {
        self.to_bytes().len() as Offset
    }

    unsafe fn from_buffer(buffer: &'a [u8], from: Offset, count: Offset) -> Self {
        let to = from + count * Self::item_size();
        let slice = &buffer[from as usize..to as usize];
        BitVec::from_bytes(slice)
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        // TODO: avoid reallocation here using normal implementation of bitvec
        let slice = &self.to_bytes();
        buffer.extend_from_slice(slice);
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
}

const HASH_ITEM_SIZE: Offset = 32;
impl<'a> SegmentField<'a> for &'a [Hash] {
    fn item_size() -> Offset {
        HASH_ITEM_SIZE
    }

    fn count(&self) -> Offset {
        self.len() as Offset
    }

    unsafe fn from_buffer(buffer: &'a [u8], from: Offset, count: Offset) -> Self {
        let to = from + count * Self::item_size();
        let slice = &buffer[(from as usize)..(to as usize)];
        ::std::slice::from_raw_parts(slice.as_ptr() as *const Hash,
                                     slice.len() / HASH_ITEM_SIZE as usize)
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        let slice = unsafe {
            ::std::slice::from_raw_parts(self.as_ptr() as *const u8,
                                         self.len() * HASH_ITEM_SIZE as usize)
        };
        buffer.extend_from_slice(slice)
    }
}
