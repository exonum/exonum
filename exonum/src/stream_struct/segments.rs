use byteorder::{ByteOrder, LittleEndian};

use messages::{BitVec, RawMessage, HEADER_SIZE, MessageBuffer, Message};
use crypto::Hash;

use super::{Error, Field, SegmentReference};

pub trait SegmentField<'a>: Sized {
    fn from_chunk(chunk: &'a [u8]) -> Self;
    fn extend_buffer(&self, buffer: &mut Vec<u8>);

    #[allow(unused_variables)]
    fn check_data(slice: &'a [u8], pos: u32) 
        -> Result<(), Error>;
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
            let start = pos as usize;
            let end = start + count as usize;
            let chunk = &buffer[start..end];
            Self::from_chunk(chunk)
        }
    }

    fn write(&self, buffer: &mut Vec<u8>, from: usize, to: usize) {
        let pos = buffer.len();
        LittleEndian::write_u32(&mut buffer[from..from + 4], pos as u32);
        self.extend_buffer(buffer);
        let count = buffer.len() - pos;
        LittleEndian::write_u32(&mut buffer[from + 4..to], count as u32);
        println!("buffer after write = {:?}", buffer)
    }

    fn check(buffer: &'a [u8], from: usize, to: usize)
         -> Result<Option<SegmentReference>, Error>
    {
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

        let end = start + count as usize;
        if end > buffer.len() {
            return Err(Error::IncorrectSegmentSize {
                position: (from + 4) as u32,
                value: count,
            });
        }

        unsafe {
            let chunk = &buffer[start..end];
            Self::check_data(chunk, from as u32)
                    .map(|_| Some(SegmentReference::new(pos, chunk.len() as u32)))
        }
    }
}

impl<'a> SegmentField<'a> for &'a str {

    fn from_chunk(slice: &'a [u8]) -> Self {
        unsafe { ::std::str::from_utf8_unchecked(slice) }
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self.as_bytes())
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
    fn from_chunk(slice: &'a [u8]) -> Self {
        RawMessage::new(MessageBuffer::from_vec(Vec::from(slice)))
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self.as_ref().as_ref())
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


// FIXME: before merge:
// 2. iteratively сheck items


impl<'a, T> SegmentField<'a> for Vec<T> where T: Field<'a> {

    //FIXME: reduce memory allocation
    fn from_chunk(slice: &'a [u8]) -> Self {
        // read vector len
        let count = u32::read(slice, 0, 4) as usize;

        let mut vec = Vec::with_capacity(count as usize);
        let mut start = 4usize;
        for _ in 0..count {
            vec.push(T::read(slice, start, start + T::field_size()));
            start += T::field_size();
        }
        vec
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        let count = self.len() as u32;
        // \TODO: avoid reallocation there, by implementing new buffer type
        // that can lock buffer pervios data;
        let mut tmpbuff = vec![0; 4 + T::field_size() * count as usize];
        // write vector len
        count.write(&mut tmpbuff, 0, 4);
        let mut start = 4;
        // write rest of fields
        for i in self.iter() {
            i.write(&mut tmpbuff, start, start + T::field_size());
            start += T::field_size();
        }

        buffer.extend_from_slice(&tmpbuff)
    }
    
    fn check_data(slice: &'a [u8], pos: u32) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a> SegmentField<'a> for BitVec {
    fn from_chunk(slice: &'a [u8]) -> Self {
        BitVec::from_bytes(slice)
    }
    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        // TODO: avoid reallocation here using normal implementation of bitvec
        let slice = &self.to_bytes();
        buffer.extend_from_slice(slice);
    }

    fn check_data(slice: &'a [u8], pos: u32) -> Result<(), Error> {
        Ok(())
    }
}



impl<'a> SegmentField<'a> for &'a [u8] {
    fn from_chunk(slice: &'a [u8]) -> Self {
        slice
    }

    fn extend_buffer(&self, buffer: &mut Vec<u8>) {
        println!("extend_buffer slice = {:?}, buffer = {:?}", self, buffer);
        buffer.extend_from_slice(self)
    }

    fn check_data(slice: &'a [u8], pos: u32) -> Result<(), Error> {
        Ok(())
    }
}

const HASH_ITEM_SIZE: usize = 32;
impl<'a> SegmentField<'a> for &'a [Hash] {
    fn from_chunk(slice: &'a [u8]) -> Self {
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

    fn check_data(slice: &'a [u8], pos: u32) -> Result<(), Error> {
        Ok(())
    }
}

