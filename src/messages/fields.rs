use std::mem;
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};

use time::{Timespec};
use byteorder::{ByteOrder, LittleEndian};

use super::super::crypto::Hash;

use super::MessageError;

pub trait MessageField<'a> {
    // TODO: use Read and Cursor
    // TODO: debug_assert_eq!(to-from == size of Self)
    fn read(buffer: &'a [u8], from: usize, to: usize) -> Self;
    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize);

    #[allow(unused_variables)]
    fn check(buffer: &'a [u8], from: usize, to: usize)
        -> Result<(), MessageError> {
        Ok(())
    }
}

impl<'a> MessageField<'a> for u32 {
    fn read(buffer: &'a [u8], from: usize, to: usize) -> u32 {
        LittleEndian::read_u32(&buffer[from..to])
    }

    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize) {
        LittleEndian::write_u32(&mut buffer[from..to], *self)
    }
}

impl<'a> MessageField<'a> for u64 {
    fn read(buffer: &'a [u8], from: usize, to: usize) -> u64 {
        LittleEndian::read_u64(&buffer[from..to])
    }

    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize) {
        LittleEndian::write_u64(&mut buffer[from..to], *self)
    }
}

impl<'a> MessageField<'a> for &'a Hash {
    fn read(buffer: &'a [u8], from: usize, _: usize) -> &'a Hash {
        unsafe {
            mem::transmute(&buffer[from])
        }
    }

    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize) {
        &mut buffer[from..to].copy_from_slice(self.as_ref());
    }
}

impl<'a> MessageField<'a> for Timespec {
    fn read(buffer: &'a [u8], from: usize, to: usize) -> Timespec {
        let nsec = LittleEndian::read_u64(&buffer[from..to]);
        Timespec {
            sec:  (nsec / 1_000_000_000) as i64,
            nsec: (nsec % 1_000_000_000) as i32,
        }
    }

    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize) {
        let nsec = (self.sec as u64) * 1_000_000_000 + self.nsec as u64;
        LittleEndian::write_u64(&mut buffer[from..to], nsec)
    }
}

impl<'a> MessageField<'a> for SocketAddr {
    // TODO: supporting IPv6

    fn read(buffer: &'a [u8], from: usize, to: usize) -> SocketAddr {
        let ip = Ipv4Addr::new(buffer[from+0], buffer[from+1],
                               buffer[from+2], buffer[from+3]);
        let port = LittleEndian::read_u16(&buffer[from+4..to]);
        SocketAddr::V4(SocketAddrV4::new(ip, port))
    }

    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize) {
        match *self {
            SocketAddr::V4(addr) => {
                &mut buffer[from..to-2].copy_from_slice(&addr.ip().octets());
            },
            SocketAddr::V6(_) => {
                // FIXME: Supporting Ipv6
                panic!("Ipv6 are currently unsupported")
            },
        }
        LittleEndian::write_u16(&mut buffer[to-2..to], self.port());
    }
}
