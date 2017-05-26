use byteorder::{ByteOrder, LittleEndian};

use std::mem;
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::time::{SystemTime, Duration, UNIX_EPOCH};

use crypto::{Hash, PublicKey, Signature};
use super::{Error, SegmentReference};

macro_rules! implement_std_field {
    ($name:ident $fn_read:expr; $fn_write:expr) => (
        impl<'a> Field<'a> for $name {
            fn field_size() -> usize {
                mem::size_of::<$name>()
            }

            fn read(buffer: &'a [u8], from: usize, to: usize) -> $name {
                $fn_read(&buffer[from..to])
            }

            fn write(&self, buffer: &mut Vec<u8>, from: usize, to: usize) {
                $fn_write(&mut buffer[from..to], *self)
            }
        }
    )
}


/// implement field helper for all 
macro_rules! implement_pod_as_ref_field {
    ($name:ident) => (
        impl<'a> Field<'a> for &'a $name {
            fn field_size() -> usize {
                mem::size_of::<$name>()
            }

            fn read(buffer: &'a [u8], from: usize, _: usize) -> &'a $name {
                unsafe { mem::transmute(&buffer[from]) }
            }

            fn write(&self, buffer: &mut Vec<u8>, from: usize, to: usize) {
                let ptr: *const $name = *self as *const $name;
                let slice = unsafe {
                    ::std::slice::from_raw_parts(ptr as * const u8, 
                                                        mem::size_of::<$name>())};
                buffer[from..to].copy_from_slice(slice);
            }
        }
    )
}

pub trait Field<'a> {
    // TODO: use Read and Cursor
    // TODO: debug_assert_eq!(to-from == size of Self)
    /// Read Field from buffer, with given position
    fn read(buffer: &'a [u8], from: usize, to: usize) -> Self;
    /// Write Field to buffer, in given position
    fn write(&self, buffer: &mut Vec<u8>, from: usize, to: usize);
    /// Field's header size
    fn field_size() -> usize;

    /// Checks if data in the buffer could be deserialized.
    /// Returns an optional segment reference, if it should consume some.
    #[allow(unused_variables)]
    fn check(buffer: &'a [u8], from: usize, to: usize) -> Result<Option<SegmentReference>, Error> {
        Ok(None)
    }
}

impl<'a> Field<'a> for bool {
    fn field_size() -> usize {
        1
    }

    fn read(buffer: &'a [u8], from: usize, _: usize) -> bool {
        buffer[from] == 1
    }

    fn write(&self, buffer: &mut Vec<u8>, from: usize, _: usize) {
        buffer[from] = if *self { 1 } else { 0 }
    }

    fn check(buffer: &'a [u8], from: usize, _: usize) -> Result<Option<SegmentReference>, Error> {
        if buffer[from] != 0 && buffer[from] != 1 {
            Err(Error::IncorrectBoolean {
                    position: from as u32,
                    value: buffer[from],
                })
        } else {
            Ok(None)
        }
    }
}

impl<'a> Field<'a> for u8 {
    fn field_size() -> usize {
        mem::size_of::<u8>()
    }

    fn read(buffer: &'a [u8], from: usize, _: usize) -> u8 {
        buffer[from]
    }

    fn write(&self, buffer: &mut Vec<u8>, from: usize, _: usize) {
        buffer[from] = *self;
    }
}


implement_std_field!{u16 LittleEndian::read_u16; LittleEndian::write_u16}
implement_std_field!{i16 LittleEndian::read_i16; LittleEndian::write_i16}
implement_std_field!{u32 LittleEndian::read_u32; LittleEndian::write_u32}
implement_std_field!{i32 LittleEndian::read_i32; LittleEndian::write_i32}
implement_std_field!{u64 LittleEndian::read_u64; LittleEndian::write_u64}
implement_std_field!{i64 LittleEndian::read_i64; LittleEndian::write_i64}

implement_pod_as_ref_field! {Signature}
implement_pod_as_ref_field! {PublicKey}
implement_pod_as_ref_field! {Hash}

impl<'a> Field<'a> for SystemTime {
    fn field_size() -> usize {
        mem::size_of::<u64>() + mem::size_of::<u32>()
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> SystemTime {
        let secs = LittleEndian::read_u64(&buffer[from..to]);
        let nanos = LittleEndian::read_u32(&buffer[from + mem::size_of_val(&secs)..to]);
        UNIX_EPOCH + Duration::new(secs, nanos)
    }

    fn write(&self, buffer: &mut Vec<u8>, from: usize, to: usize) {
        let duration = self.duration_since(UNIX_EPOCH).unwrap();
        let secs = duration.as_secs();
        let nanos = duration.subsec_nanos();
        LittleEndian::write_u64(&mut buffer[from..to - mem::size_of_val(&nanos)], secs);
        LittleEndian::write_u32(&mut buffer[from + mem::size_of_val(&secs)..to], nanos);
    }
}

impl<'a> Field<'a> for SocketAddr {
    fn field_size() -> usize {
        // reserve space for future compatibility
        32
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> SocketAddr {
        let mut octets = [0u8; 4];
        octets.copy_from_slice(&buffer[from..from + 4]);
        let ip = Ipv4Addr::from(octets);
        let port = LittleEndian::read_u16(&buffer[from + 4..to]);
        SocketAddr::V4(SocketAddrV4::new(ip, port))
    }

    fn write(&self, buffer: &mut Vec<u8>, from: usize, to: usize) {
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
