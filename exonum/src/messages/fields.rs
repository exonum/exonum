use std::mem;
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};

use time::Timespec;
use byteorder::{ByteOrder, LittleEndian};

use super::super::crypto::{Hash, PublicKey};

use super::{Error, Message};

pub trait Field<'a> {
    // TODO: use Read and Cursor
    // TODO: debug_assert_eq!(to-from == size of Self)
    fn read(buffer: &'a [u8], from: usize, to: usize) -> Self;
    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize);
    fn field_size() -> usize;

    #[allow(unused_variables)]
    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a> Field<'a> for bool {
    fn field_size() -> usize {
        1
    }

    fn read(buffer: &'a [u8], from: usize, _: usize) -> bool {
        buffer[from] == 1
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, _: usize) {
        buffer[from] = if *self { 1 } else { 0 }
    }

    fn check(buffer: &'a [u8], from: usize, _: usize) -> Result<(), Error> {
        if buffer[from] != 0 && buffer[from] != 1 {
            Err(Error::IncorrectBoolean {
                position: from as u32,
                value: buffer[from],
            })
        } else {
            Ok(())
        }
    }
}

impl<'a> Field<'a> for u16 {
    fn field_size() -> usize {
        2
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> u16 {
        LittleEndian::read_u16(&buffer[from..to])
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        LittleEndian::write_u16(&mut buffer[from..to], *self)
    }
}

impl<'a> Field<'a> for u32 {
    fn field_size() -> usize {
        4
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> u32 {
        LittleEndian::read_u32(&buffer[from..to])
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        LittleEndian::write_u32(&mut buffer[from..to], *self)
    }
}

impl<'a> Field<'a> for u64 {
    fn field_size() -> usize {
        8
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> u64 {
        LittleEndian::read_u64(&buffer[from..to])
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        LittleEndian::write_u64(&mut buffer[from..to], *self)
    }
}

impl<'a> Field<'a> for i64 {
    fn field_size() -> usize {
        8
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> i64 {
        LittleEndian::read_i64(&buffer[from..to])
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        LittleEndian::write_i64(&mut buffer[from..to], *self)
    }
}

impl<'a> Field<'a> for &'a Hash {
    fn field_size() -> usize {
        32
    }

    fn read(buffer: &'a [u8], from: usize, _: usize) -> &'a Hash {
        unsafe { mem::transmute(&buffer[from]) }
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        buffer[from..to].copy_from_slice(self.as_ref());
    }
}

impl<'a> Field<'a> for &'a PublicKey {
    fn field_size() -> usize {
        32
    }

    fn read(buffer: &'a [u8], from: usize, _: usize) -> &'a PublicKey {
        unsafe { mem::transmute(&buffer[from]) }
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        buffer[from..to].copy_from_slice(self.as_ref());
    }
}

impl<'a> Field<'a> for Timespec {
    fn field_size() -> usize {
        8
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> Timespec {
        let nsec = LittleEndian::read_u64(&buffer[from..to]);
        Timespec {
            sec: (nsec / 1_000_000_000) as i64,
            nsec: (nsec % 1_000_000_000) as i32,
        }
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        let nsec = (self.sec as u64) * 1_000_000_000 + self.nsec as u64;
        LittleEndian::write_u64(&mut buffer[from..to], nsec)
    }
}

impl<'a> Field<'a> for SocketAddr {
    fn field_size() -> usize {
        32
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> SocketAddr {
        let mut octets = [0u8; 4];
        octets.copy_from_slice(&buffer[from..from + 4]);
        let ip = Ipv4Addr::from(octets);
        let port = LittleEndian::read_u16(&buffer[from + 4..to]);
        SocketAddr::V4(SocketAddrV4::new(ip, port))
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        match *self {
            SocketAddr::V4(addr) => {
                buffer[from..to - 2].copy_from_slice(&addr.ip().octets());
            }
            SocketAddr::V6(_) => {
                // FIXME: Supporting Ipv6
                panic!("Ipv6 are currently unsupported")
            }
        }
        LittleEndian::write_u16(&mut buffer[to - 2..to], self.port());
    }
}

pub trait SegmentField<'a> {
    fn from_slice(slice: &'a [u8]) -> Self;
    fn as_slice(&self) -> &'a [u8];
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
        1
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
        buffer.extend_from_slice(self.as_slice());
    }

    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<(), Error> {
        let pos = LittleEndian::read_u32(&buffer[from..from + 4]);
        let count = LittleEndian::read_u32(&buffer[from + 4..to]);

        if count == 0 {
            return Ok(());
        }

        let start = pos as usize;

        if start < from + 8 {
            return Err(Error::IncorrectSegmentRefference {
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

impl<'a> Field<'a> for Vec<&'a [u8]> {
    fn field_size() -> usize {
        1
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> Vec<&'a [u8]> {
        unsafe {
            debug_assert_eq!(to - from, 8);
            let pos = LittleEndian::read_u32(&buffer[from..from + 4]) as usize;
            let count = LittleEndian::read_u32(&buffer[from + 4..to]) as usize;
            println!("Array: pos: {}, count: {}, buffer_len: {}", pos, count, buffer.len());

            let segments = &buffer[pos..pos + count * 8];
            
            let mut vec = Vec::new();
            for i in 0..count {
                let from = i * 8;
                let pos = LittleEndian::read_u32(&segments[from..from + 4]);
                let count = LittleEndian::read_u32(&segments[from + 4..from + 8]);
                println!("segment: pos: {}, count: {}", pos, count);

                let ptr = buffer.as_ptr().offset(pos as isize);
                let len = count as usize;
                let slice_ptr = ::std::slice::from_raw_parts(ptr as *const u8, len);
                vec.push(slice_ptr);
            }
            vec
        }
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        debug_assert_eq!(to - from, 8);

        let pos = buffer.len();
        LittleEndian::write_u32(&mut buffer[from..from + 4], pos as u32);
        LittleEndian::write_u32(&mut buffer[from + 4..to], self.len() as u32);

        buffer.resize(pos + self.len() * 8 + 8, 0);
        println!("pos: {}, to: {}, from: {}, len: {}", pos, to, from, buffer.len());
        //debug_assert_eq!(pos + (to - from), buffer.len());
        // Write segment positions
        println!("Write: resize buf to {}", buffer.len());
        let mut from = pos;
        let mut pos = buffer.len();
        for segment in self.iter() {
            let len = segment.len();
            LittleEndian::write_u32(&mut buffer[from..from + 4], pos as u32);
            LittleEndian::write_u32(&mut buffer[from + 4..from + 8], len as u32);
            println!("Write segment ptr: from:{}, len: {}, total_len: {}", from, len, pos);
            from += 8;
            pos += len;
        }
        // Write segment bodies
        for segment in self.iter() {
            buffer.extend_from_slice(segment);
            println!("Write: resize buf to {}", buffer.len());            
        }
    }

    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<(), Error> {
        // let pos = LittleEndian::read_u32(&buffer[from..from + 4]);
        // let count = LittleEndian::read_u32(&buffer[from + 4..to]);

        // if count == 0 {
        //     return Ok(());
        // }

        // let start = pos as usize;

        // if start < from + 8 {
        //     return Err(Error::IncorrectSegmentRefference {
        //         position: from as u32,
        //         value: pos,
        //     });
        // }

        // let end = start + Self::item_size() * (count as usize);

        // if end > buffer.len() {
        //     return Err(Error::IncorrectSegmentSize {
        //         position: (from + 4) as u32,
        //         value: count,
        //     });
        // }

        // unsafe {
        //     let ptr = buffer.as_ptr().offset(pos as isize);
        //     let len = (count as usize) * Self::item_size();
        //     Self::check_data(::std::slice::from_raw_parts(ptr as *const u8, len),
        //                      from as u32)
        // }
        Ok(())
    }   
}

impl<'a> SegmentField<'a> for &'a [u8] {
    fn item_size() -> usize {
        1
    }

    fn from_slice(slice: &'a [u8]) -> Self {
        slice
    }

    fn as_slice(&self) -> &'a [u8] {
        self
    }

    fn count(&self) -> u32 {
        self.len() as u32
    }
}

impl<'a> SegmentField<'a> for &'a [u16] {
    fn item_size() -> usize {
        mem::size_of::<u16>()
    }

    fn from_slice(slice: &'a [u8]) -> Self {
        unsafe {
            ::std::slice::from_raw_parts(slice.as_ptr() as *const u16,
                                         slice.len() / Self::item_size())
        }
    }

    fn as_slice(&self) -> &'a [u8] {
        unsafe {
            ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * Self::item_size())
        }
    }

    fn count(&self) -> u32 {
        self.len() as u32
    }
}

impl<'a> SegmentField<'a> for &'a [u32] {
    fn item_size() -> usize {
        mem::size_of::<u32>()
    }

    fn from_slice(slice: &'a [u8]) -> Self {
        unsafe {
            ::std::slice::from_raw_parts(slice.as_ptr() as *const u32,
                                         slice.len() / Self::item_size())
        }
    }

    fn as_slice(&self) -> &'a [u8] {
        unsafe {
            ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * Self::item_size())
        }
    }

    fn count(&self) -> u32 {
        self.len() as u32
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

    fn as_slice(&self) -> &'a [u8] {
        unsafe {
            ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * Self::item_size())
        }
    }

    fn count(&self) -> u32 {
        self.len() as u32
    }
}

impl<'a> SegmentField<'a> for &'a str {
    fn item_size() -> usize {
        1
    }

    fn from_slice(slice: &'a [u8]) -> Self {
        unsafe { ::std::str::from_utf8_unchecked(slice) }
    }

    fn as_slice(&self) -> &'a [u8] {
        self.as_bytes()
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

// impl<'a, T> SegmentField<'a> for &'a [T] where T: Field<'a> {
//     fn item_size() -> usize {
//         T::field_size()
//     }

//     fn from_slice(slice: &'a [u8]) -> Self {
//         unsafe { ::std::slice::from_raw_parts(slice.as_ptr() as *const T, slice.len() / Self::item_size()) }
//     }

//     fn as_slice(&self) -> &'a [u8] {
//         unsafe { ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * Self::item_size()) }
//     }

//     fn count(&self) -> u32 {
//         self.len() as u32
//     }
// }

#[test]
fn test_str_segment() {
    let mut buf = vec![0; 8];
    let s = "test юникодной строчки efw_adqq ss/adfq";
    Field::write(&s, &mut buf, 0, 8);
    <&str as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <&str as Field>::check(&buf2, 0, 8).unwrap();
    let s2: &str = Field::read(&buf2, 0, 8);
    assert_eq!(s2, s);
}

#[test]
fn test_u16_segment() {
    let mut buf = vec![0; 8];
    let s = [1u16, 3, 10, 15, 23, 4, 45];
    Field::write(&s.as_ref(), &mut buf, 0, 8);
    <&[u16] as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <&[u16] as Field>::check(&buf2, 0, 8).unwrap();
    let s2: &[u16] = Field::read(&buf2, 0, 8);
    assert_eq!(s2, s.as_ref());
}

#[test]
fn test_u32_segment() {
    let mut buf = vec![0; 8];
    let s = [1u32, 3, 10, 15, 23, 4, 45];
    Field::write(&s.as_ref(), &mut buf, 0, 8);
    <&[u32] as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <&[u32] as Field>::check(&buf2, 0, 8).unwrap();
    let s2: &[u32] = Field::read(&buf2, 0, 8);
    assert_eq!(s2, s.as_ref());
}

#[test]
fn test_segments_of_segments() {
    let mut buf = vec![0; 8];
    let v1 = [1u8, 2, 3];
    let v2 = [1u8, 3];
    let v3 = [2u8, 5, 2, 3, 56, 3];

    let dat = vec![v1.as_ref(), v2.as_ref(), v3.as_ref()];
    Field::write(&dat, &mut buf, 0, 8);
    <Vec<&[u8]> as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <Vec<&[u8]> as Field>::check(&buf2, 0, 8).unwrap();
    let dat2: Vec<&[u8]> = Field::read(&buf2, 0, 8);
    assert_eq!(dat2, dat);
}