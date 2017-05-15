use byteorder::{ByteOrder, LittleEndian};

use messages::{BitVec, RawMessage, HEADER_SIZE, MessageBuffer};
use crypto::Hash;

use super::{Result, Error, Field, SegmentReference};

pub trait SegmentField<'a>: Sized {
    fn item_size() -> usize;
    fn count(&self) -> usize;
    fn from_buffer(buffer: &'a [u8], from: usize, count: usize) -> Self;
    fn extend_buffer(&self, buffer: &mut Vec<u8>);

    #[allow(unused_variables)]
    fn check_data(buffer: &'a [u8], from: usize, count: usize) -> Result {
        let to = from + count * Self::item_size();
        Ok(Some(SegmentReference::new(from as u32, to as u32)))
    }
}

impl<'a, T> Field<'a> for T
    where T: SegmentField<'a>
{
    fn field_size() -> usize {
        8
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> T {
        let pos = LittleEndian::read_u32(&buffer[from..from + 4]);
        let count = LittleEndian::read_u32(&buffer[from + 4..to]);
        Self::from_buffer(buffer, pos as usize, count as usize)
    }

    fn write(&self, buffer: &mut Vec<u8>, from: usize, to: usize) {
        let pos = buffer.len() as u32;
        LittleEndian::write_u32(&mut buffer[from..from + 4], pos);
        LittleEndian::write_u32(&mut buffer[from + 4..to], self.count() as u32);
        self.extend_buffer(buffer);
        
    }

    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result {
        let pos = LittleEndian::read_u32(&buffer[from..from + 4]);
        let count = LittleEndian::read_u32(&buffer[from + 4..to]);

        if count == 0 {
            return Ok(None);
        }

        let start = pos as usize;

        if start < from + 8 {
            return Err(Error::IncorrectSegmentReference {
                position: from as u32,
                value: pos,
            });
        }

        let end = start + (count as usize * Self::item_size());
        if end > buffer.len() {
            return Err(Error::IncorrectSegmentSize {
                position: (from + 4) as u32,
                value: count,
            });
        }

        Self::check_data(buffer, start, count as usize)
    }
}

impl<'a> SegmentField<'a> for &'a str {
    
    fn item_size() -> usize {
        1
    }

    fn count(&self) -> usize {
        self.as_bytes().len()
    }

    fn from_buffer(buffer: &'a [u8], from: usize, count: usize) -> Self {
        let to = from + count * Self::item_size();
        let slice = &buffer[from..to];
        unsafe { ::std::str::from_utf8_unchecked(slice) }
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self.as_bytes())
    }

    fn check_data(buffer: &'a [u8], from: usize, count: usize) -> Result {
        let to = from + count * Self::item_size();
        let slice = &buffer[from..to];
        if let Err(e) = ::std::str::from_utf8(slice) {
            return Err(Error::Utf8 {
                position: from as u32,
                error: e,
            });
        }

        let to = from + count * Self::item_size();
        Ok(Some(SegmentReference::new(from as u32, to as u32)))
    }
}

impl<'a> SegmentField<'a> for RawMessage {

    fn item_size() -> usize {
        1
    }

    fn count(&self) -> usize {
        self.as_ref().as_ref().len()
    }

    fn from_buffer(buffer: &'a [u8], from: usize, to: usize) -> Self {
        let to = from + to * Self::item_size();
        let slice = &buffer[from..to];
        RawMessage::new(MessageBuffer::from_vec(Vec::from(slice)))
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        println!("extend buffer={:?}, \n extend={:?}", buffer, self.as_ref().as_ref());
        buffer.extend_from_slice(self.as_ref().as_ref())
    }

    fn check_data(buffer: &'a [u8], from: usize, count: usize) -> Result {
        let to = from + count * Self::item_size();
        let slice = &buffer[from..to];
        println!("from ={:?}, count = {:?}, slice = {:?}", from, count, slice);
        if slice.len() < HEADER_SIZE {
            return Err(Error::UnexpectedlyShortRawMessage {
                position: from as u32,
                size: slice.len() as u32
            });
        }
        let actual_size = slice.len() as u32;
        let declared_size = LittleEndian::read_u32(&slice[6..10]);
        if actual_size != declared_size {
            return Err(Error::IncorrectSizeOfRawMessage {
                position: from as u32,
                actual_size: slice.len() as u32,
                declared_size: declared_size
            });
        }
        Ok(Some(SegmentReference::new(from as u32, to as u32)))
    }
}

impl<'a, T> SegmentField<'a> for Vec<T> where T: Field<'a> {

    fn item_size() -> usize {
        T::field_size()
    }

    fn count(&self) -> usize {
        self.len()
    }

    // TODO: implement different
    // for Vec<T> where T: Field,
    // for Vec<T> where T = u8
    // but this is possible only after specialization land
    fn from_buffer(buffer: &'a [u8], from: usize, count: usize) -> Self {
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
        let mut start = buffer.len();
        buffer.resize(start + self.count() * Self::item_size(), 0);
        // write rest of fields
        for i in self.iter() {
            i.write(&mut buffer, start, start + Self::item_size());
            println!("EXTENDBUFFER = {:?}", buffer);
            start += Self::item_size();
        }
    }
    fn check_data(buffer: &'a [u8], from: usize, count: usize) -> Result {
        let mut start = from;
        let header = (start + count * Self::item_size()) as u32;
        let mut last_data = header ;
        
        for _ in 0..count {
            println!("HEADER = {:?}, start = {:?}", last_data, start);
            T::check(buffer, start, start + Self::item_size())?
                .map_or(Ok(()), |mut e| e.check_segment(header as u32, &mut last_data))?;
            start += Self::item_size();
        }
        Ok((Some(SegmentReference::new(from as u32, last_data))))
    }
    
}

impl<'a> SegmentField<'a> for BitVec {

    fn item_size() -> usize {
        1
    }

    // TODO: reduce memory allocation
    fn count(&self) -> usize {
        self.to_bytes().len()
    }

    fn from_buffer(buffer: &'a [u8], from: usize, count: usize) -> Self {
        let to = from + count * Self::item_size();
        let slice = &buffer[from..to];
        BitVec::from_bytes(slice)
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        // TODO: avoid reallocation here using normal implementation of bitvec
        let slice = &self.to_bytes();
        buffer.extend_from_slice(slice);
    }
}

impl<'a> SegmentField<'a> for &'a [u8] {

    fn item_size() -> usize {
        1
    }

    fn count(&self) -> usize {
        self.len()
    }

    fn from_buffer(buffer: &'a [u8], from: usize, count: usize) -> Self {
        let to = from + count * Self::item_size();
        let slice = &buffer[from..to];
        slice
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self)
    }
}

const HASH_ITEM_SIZE: usize = 32;
impl<'a> SegmentField<'a> for &'a [Hash] {

    fn item_size() -> usize {
        HASH_ITEM_SIZE
    }

    fn count(&self) -> usize {
        self.len()
    }

    fn from_buffer(buffer: &'a [u8], from: usize, count: usize) -> Self {
        let to = from + count * Self::item_size();
        let slice = &buffer[from..to];
        unsafe {
            ::std::slice::from_raw_parts(slice.as_ptr() as *const Hash,
                                         slice.len() / HASH_ITEM_SIZE)
        }
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        let slice = unsafe {
            ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * HASH_ITEM_SIZE)
        };
        buffer.extend_from_slice(&slice)
    }

}

