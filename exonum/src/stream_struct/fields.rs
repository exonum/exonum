use byteorder::{ByteOrder, LittleEndian};

use std::mem;
use std::sync::Arc;
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::time::{SystemTime, Duration, UNIX_EPOCH};

use crypto::{Hash, PublicKey, Signature};
use super::Error;
use messages::{RawMessage, MessageBuffer, BitVec, FromRaw};

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
