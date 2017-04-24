use byteorder::{ByteOrder, LittleEndian};

use std::mem;
use std::sync::Arc;
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::time::{SystemTime, Duration, UNIX_EPOCH};

use crypto::{Hash, PublicKey, Signature};
use super::{Error, RawMessage, MessageBuffer, BitVec, FromRaw};

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

impl<'a> Field<'a> for u8 {
    fn field_size() -> usize {
        1
    }

    fn read(buffer: &'a [u8], from: usize, _: usize) -> u8 {
        buffer[from]
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, _: usize) {
        buffer[from] = *self;
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

impl<'a> Field<'a> for &'a Signature {
    fn field_size() -> usize {
        32
    }

    fn read(buffer: &'a [u8], from: usize, _: usize) -> &'a Signature {
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

impl<'a> Field<'a> for SystemTime {
    fn field_size() -> usize {
        mem::size_of::<u64>() + mem::size_of::<u32>()
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> SystemTime {
        let secs = LittleEndian::read_u64(&buffer[from..to]);
        let nanos = LittleEndian::read_u32(&buffer[from + mem::size_of_val(&secs)..to]);
        UNIX_EPOCH + Duration::new(secs, nanos)
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        let duration = self.duration_since(UNIX_EPOCH).unwrap();
        let secs = duration.as_secs();
        let nanos = duration.subsec_nanos();
        LittleEndian::write_u64(&mut buffer[from..to - mem::size_of_val(&nanos)], secs);
        LittleEndian::write_u32(&mut buffer[from + mem::size_of_val(&secs)..to], nanos);
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

impl<'a> SegmentField<'a> for &'a [u64] {
    fn item_size() -> usize {
        mem::size_of::<u64>()
    }

    fn from_slice(slice: &'a [u8]) -> Self {
        unsafe {
            ::std::slice::from_raw_parts(slice.as_ptr() as *const u64,
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

impl<'a> SegmentField<'a> for &'a [f64] {
    fn item_size() -> usize {
        mem::size_of::<f64>()
    }

    fn from_slice(slice: &'a [u8]) -> Self {
        unsafe {
            ::std::slice::from_raw_parts(slice.as_ptr() as *const f64,
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

// TODO: Make this more generic and support storing not only &[u8].
// TODO: Remove magic constants and additional allocations.
impl<'a> Field<'a> for Vec<&'a [u8]> {
    fn field_size() -> usize {
        1
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> Vec<&'a [u8]> {
        debug_assert_eq!(to - from, 8);

        let pos = LittleEndian::read_u32(&buffer[from..from + 4]) as usize;
        let count = LittleEndian::read_u32(&buffer[from + 4..to]) as usize;

        let mut vec = Vec::new();
        for i in 0..count {
            let from = pos + i * 8;
            let slice = <&[u8] as Field>::read(buffer, from, from + 8);
            vec.push(slice);
        }
        vec
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        debug_assert_eq!(to - from, 8);

        let pos = buffer.len();
        LittleEndian::write_u32(&mut buffer[from..from + 4], pos as u32);
        LittleEndian::write_u32(&mut buffer[from + 4..to], self.len() as u32);
        buffer.resize(pos + self.len() * 8, 0);

        // Write segment headers
        let mut from = pos;
        let mut pos = buffer.len();
        for segment in self.iter() {
            let count = segment.count();
            LittleEndian::write_u32(&mut buffer[from..from + 4], pos as u32);
            LittleEndian::write_u32(&mut buffer[from + 4..from + 8], count);

            from += 8;
            pos += count as usize;
        }

        // Write segment bodies
        for segment in self.iter() {
            buffer.extend_from_slice(segment.as_slice());
        }
    }

    // TODO: Remove recursive check?
    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<(), Error> {
        let pos = LittleEndian::read_u32(&buffer[from..from + 4]) as usize;
        let count = LittleEndian::read_u32(&buffer[from + 4..to]) as usize;

        if count == 0 {
            return Ok(());
        }

        if pos < from + 8 {
            return Err(Error::IncorrectSegmentReference {
                position: from as u32,
                value: pos as u32,
            });
        }

        let end = pos + 8 * count;
        if end > buffer.len() {
            return Err(Error::IncorrectSegmentSize {
                position: (from + 4) as u32,
                value: count as u32,
            });
        }

        for i in 0..count {
            let from = pos + i * 8;
            <&[u8] as Field>::check(buffer, from, from + 8)?;
        }
        Ok(())
    }
}

impl<'a> Field<'a> for Vec<RawMessage> {
    fn field_size() -> usize {
        1
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> Vec<RawMessage> {
        let raw: Vec<&[u8]> = Field::read(buffer, from, to);
        raw.into_iter()
            .map(|x| Arc::new(MessageBuffer::from_vec(x.to_vec())))
            .collect()
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        let raw = self.into_iter()
            .map(|x| x.as_ref().as_ref())
            .collect::<Vec<&[u8]>>();
        Field::write(&raw, buffer, from, to);
    }

    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<(), Error> {
        // TODO check messages as messages
        <Vec<&[u8]> as Field>::check(buffer, from, to)
    }
}

impl<'a, T> Field<'a> for Vec<T>
    where T: FromRaw
{
    fn field_size() -> usize {
        1
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> Vec<T> {
        let raw: Vec<RawMessage> = Field::read(buffer, from, to);
        raw.into_iter()
            .map(|x| T::from_raw(x).unwrap()) //FIXME remove unwrap
            .collect()
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        let raw = self.into_iter()
            .map(|x| x.raw().as_ref().as_ref())
            .collect::<Vec<&[u8]>>();
        Field::write(&raw, buffer, from, to);
    }

    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<(), Error> {
        <Vec<RawMessage> as Field>::check(buffer, from, to)?;
        let raw_messages: Vec<RawMessage> = Field::read(buffer, from, to);
        for raw in raw_messages {
            T::from_raw(raw)?;
        }
        Ok(())
    }
}

impl<'a> Field<'a> for Vec<u8> {
    fn field_size() -> usize {
        8
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> Vec<u8> {
        let data = <&[u8] as Field>::read(buffer, from, to);
        data.to_vec()
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        <&[u8] as Field>::write(&self.as_slice(), buffer, from, to);
    }

    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<(), Error> {
        <&[u8] as Field>::check(buffer, from, to)?;
        Ok(())
    }
}

impl<'a> Field<'a> for RawMessage {
    fn field_size() -> usize {
        1
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> RawMessage {
        let data = <Vec<u8> as Field>::read(buffer, from, to);
        Arc::new(MessageBuffer::from_vec(data))
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        let self_slice = self.as_ref().as_ref();
        <&[u8] as Field>::write(&self_slice, buffer, from, to);
    }

    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<(), Error> {
        <&[u8] as Field>::check(buffer, from, to)?;
        Ok(())
    }
}

impl<'a> Field<'a> for BitVec {
    fn field_size() -> usize {
        8
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> BitVec {
        let data = <&[u8] as Field>::read(buffer, from, to);
        BitVec::from_bytes(data)
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        // TODO avoid reallocation
        let vec = self.to_bytes();
        let slice = vec.as_slice();
        <&[u8] as Field>::write(&slice, buffer, from, to);
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
