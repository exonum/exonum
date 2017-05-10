use byteorder::{ByteOrder, LittleEndian};

use messages::{BitVec, RawMessage, HEADER_SIZE, MessageBuffer, Message};
use crypto::Hash;

use super::{Error, Field};

pub trait SegmentField<'a> {
    fn from_slice(slice: &'a [u8]) -> Self;
    fn extend_buffer(&self, buffer: &mut Vec<u8>);
    fn count(&self) -> u32;
    fn item_size() -> usize;

    #[allow(unused_variables)]
    fn check_data(slice: &'a [u8], pos: u32) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, T> Field<'a> for T
    where T: SegmentField<'a>
{
    fn field_size() -> usize {
        8
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> T {
        unsafe {
            let pos = LittleEndian::read_u32(&buffer[from..from + 4]);
            let count = LittleEndian::read_u32(&buffer[from + 4..to]);
            let ptr = buffer.as_ptr().offset(pos as isize);
            let len = (count as usize) * Self::item_size();
            Self::from_slice(::std::slice::from_raw_parts(ptr as *const u8, len))
        }
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        let pos = buffer.len();
        LittleEndian::write_u32(&mut buffer[from..from + 4], pos as u32);
        LittleEndian::write_u32(&mut buffer[from + 4..to], self.count());
        self.extend_buffer(buffer);
    }

    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<(), Error> {
        let pos = LittleEndian::read_u32(&buffer[from..from + 4]);
        let count = LittleEndian::read_u32(&buffer[from + 4..to]);

        if count == 0 {
            return Ok(());
        }

        let start = pos as usize;

        if start < from + 8 {
            return Err(Error::IncorrectSegmentReference {
                position: from as u32,
                value: pos,
            });
        }

        let end = start + Self::item_size() * (count as usize);

        if end > buffer.len() {
            return Err(Error::IncorrectSegmentSize {
                position: (from + 4) as u32,
                value: count,
            });
        }

        unsafe {
            let ptr = buffer.as_ptr().offset(pos as isize);
            let len = (count as usize) * Self::item_size();
            Self::check_data(::std::slice::from_raw_parts(ptr as *const u8, len),
                             from as u32)
        }
    }
}

impl<'a> SegmentField<'a> for &'a str {
    fn item_size() -> usize {
        1
    }

    fn from_slice(slice: &'a [u8]) -> Self {
        unsafe { ::std::str::from_utf8_unchecked(slice) }
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self.as_bytes())
    }

    fn count(&self) -> u32 {
        self.as_bytes().len() as u32
    }

    fn check_data(slice: &'a [u8], pos: u32) -> Result<(), Error> {
        if let Err(e) = ::std::str::from_utf8(slice) {
            return Err(Error::Utf8 {
                position: pos,
                error: e,
            });
        }
        Ok(())
    }
}

impl<'a> SegmentField<'a> for RawMessage {
    fn item_size() -> usize {
        1
    }

    fn from_slice(slice: &'a [u8]) -> Self {
        RawMessage::new(MessageBuffer::from_vec(Vec::from(slice)))
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self.as_ref().as_ref())
    }

    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn check_data(slice: &'a [u8], pos: u32) -> Result<(), Error> {
        if slice.len() < HEADER_SIZE {
            return Err(Error::UnexpectedlyShortRawMessage {
                position: pos,
                size: slice.len() as u32
            });
        }
        let actual_size = slice.len() as u32;
        let declared_size = LittleEndian::read_u32(&slice[4..8]);
        if actual_size != declared_size {
            return Err(Error::IncorrectSizeOfRawMessage {
                position: pos,
                actual_size: slice.len() as u32,
                declared_size: declared_size
            });
        }
        Ok(())
    }
}


// FIXME before merge:
// 1. iteratively read items into Vec
// 2. iteratively сheck items
// 3. iteratively write items

impl<'a, T> SegmentField<'a> for Vec<T> where T: Clone + Field<'a> {
    fn item_size() -> usize {
        T::field_size()
    }

    fn from_slice(slice: &'a [u8]) -> Self {
        let slice:&[T] = unsafe { ::std::slice::from_raw_parts(slice.as_ptr() as *const u8, slice.len() / Self::item_size()) };
        Vec::from(slice)
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        let slice = unsafe { ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * Self::item_size()) };
        buffer.extend_from_slice(slice);
    }

    fn count(&self) -> u32 {
        self.len() as u32
    }
}

impl<'a> SegmentField<'a> for BitVec {
    fn from_slice(slice: &'a [u8]) -> Self {
        BitVec::from_bytes(slice)
    }
    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        // TODO: avoid reallocation here using normal implementation of bitvec
        buffer.extend_from_slice(&self.to_bytes())
    }
    fn count(&self) -> u32 {
        self.blocks().len() as u32
    }
    fn item_size() -> usize {
        32 / 8 // BitBlock = u32
    }
}

impl<'a> SegmentField<'a> for &'a [Hash] {
    fn item_size() -> usize {
        32
    }

    fn from_slice(slice: &'a [u8]) -> Self {
        unsafe {
            ::std::slice::from_raw_parts(slice.as_ptr() as *const Hash,
                                         slice.len() / Self::item_size())
        }
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        let slice = unsafe {
            ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * Self::item_size())
        };
        // TODO: avoid reallocation here using normal implementation of bitvec
        buffer.extend_from_slice(&slice)
    }

    fn count(&self) -> u32 {
        self.len() as u32
    }
}
