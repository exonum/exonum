use byteorder::{ByteOrder, LittleEndian};

use std::mem;
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::time::{SystemTime, Duration, UNIX_EPOCH};

use crypto::{Hash, PublicKey, Signature};
use super::{Error, SegmentReference, CheckedOffset, Offset};


/// implement field for all types that has writer and reader functions
///
/// - reader signature is `fn (&[u8]) -> T`
/// - writer signature is `fn (&mut [u8], T)`
#[macro_export]
macro_rules! implement_std_field {
    ($name:ident $fn_read:expr; $fn_write:expr) => (
        impl<'a> Field<'a> for $name {
            fn field_size() -> Offset {
                mem::size_of::<$name>() as Offset
            }

            unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> $name {
                $fn_read(&buffer[from as usize..to as usize])
            }

            fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
                $fn_write(&mut buffer[from as usize..to as usize], *self)
            }
            fn check(buffer: &'a [u8],
                    from: CheckedOffset,
                    to: CheckedOffset)
                    -> Result<Option<SegmentReference>, Error> {
                let len = buffer.len();
                if len < to.unchecked_offset() as usize {
                    return Err(Error::UnexpectedlyShortPayload {
                        actual_size: len as Offset,
                        minimum_size: to.unchecked_offset(),
                    });
                }
                if (to - from)?.unchecked_offset() != Self::field_size() {
                    return Err(Error::FieldSizeMismatch {
                        actual_size: (to - from)?.unchecked_offset(),
                        expected_size: Self::field_size(),
                    });
                }
                Ok(None)
            }
        }
    )
}

/// Implement field helper for all POD types
/// it writes POD type as bytearray in place.
///
/// **Beware of platform specific data representation.**
#[macro_export]
macro_rules! implement_pod_as_ref_field {
    ($name:ident) => (
        impl<'a> Field<'a> for &'a $name {
            fn field_size() -> Offset {
                ::std::mem::size_of::<$name>() as Offset
            }

            unsafe fn read(buffer: &'a [u8], from: Offset, _: Offset) -> &'a $name {
                ::std::mem::transmute(&buffer[from as usize])
            }

            fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
                let ptr: *const $name = *self as *const $name;
                let slice = unsafe {
                    ::std::slice::from_raw_parts(ptr as * const u8,
                                                        ::std::mem::size_of::<$name>())};
                buffer[from as usize..to as usize].copy_from_slice(slice);
            }

            fn check(buffer: &'a [u8],
                    from: CheckedOffset,
                    to: CheckedOffset)
                    -> Result<Option<SegmentReference>, Error> {
                let len = buffer.len();
                if len < to.unchecked_offset() as usize {
                    return Err(Error::UnexpectedlyShortPayload {
                        actual_size: len as Offset,
                        minimum_size: to.unchecked_offset(),
                    });
                }
                if (to - from)?.unchecked_offset() != Self::field_size() {
                    return Err(Error::FieldSizeMismatch {
                        actual_size: (to - from)?.unchecked_offset(),
                        expected_size: Self::field_size(),
                    });
                }
                Ok(None)
            }
        }


    )
}


//\TODO this check should be rewritted as part of buffer implementation.
macro_rules! check_field_size {
    ($buffer:ident $from:expr; $to:expr) => {
        {
        let len = $buffer.len();
        if len < $to.unchecked_offset() as usize {
            return Err(Error::UnexpectedlyShortPayload {
                actual_size: len as Offset,
                minimum_size: $to.unchecked_offset(),
            });
        }

        if ($to - $from)?.unchecked_offset() != Self::field_size() {
            return Err(Error::FieldSizeMismatch {
                actual_size: ($to - $from)?.unchecked_offset(),
                expected_size: Self::field_size(),
            });
        }
        }
    }
}

/// Trait for all types that could be a field in `stream_struct`.
pub trait Field<'a> {
    // TODO: use Read and Cursor
    // TODO: debug_assert_eq!(to-from == size of Self)

    /// Read Field from buffer, with given position,
    /// beware of memory unsafety,
    /// you should `check` `Field` before `read`.
    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self;

    /// Write Field to buffer, in given position
    /// `write` doesn't lead to memory unsafety.
    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset);
    /// Field's header size
    fn field_size() -> Offset;

    /// Checks if data in the buffer could be deserialized.
    /// Returns an optional segment reference, if it should consume some.
    #[allow(unused_variables)]
    fn check(buffer: &'a [u8],
             from: CheckedOffset,
             to: CheckedOffset)
             -> Result<Option<SegmentReference>, Error>;
}

impl<'a> Field<'a> for bool {
    fn field_size() -> Offset {
        1
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, _: Offset) -> bool {
        buffer[from as usize] == 1
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, _: Offset) {
        buffer[from as usize] = if *self { 1 } else { 0 }
    }

    fn check(buffer: &'a [u8],
             from: CheckedOffset,
             to: CheckedOffset)
             -> Result<Option<SegmentReference>, Error> {
        check_field_size!{buffer from; to};
        let from: Offset = from.unchecked_offset();
        if buffer[from as usize] != 0 && buffer[from as usize] != 1 {
            Err(Error::IncorrectBoolean {
                    position: from,
                    value: buffer[from as usize],
                })
        } else {
            Ok(None)
        }
    }
}

impl<'a> Field<'a> for u8 {
    fn field_size() -> Offset {
        mem::size_of::<Self>() as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, _: Offset) -> u8 {
        buffer[from as usize]
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, _: Offset) {
        buffer[from as usize] = *self;
    }

    fn check(buffer: &'a [u8],
             from: CheckedOffset,
             to: CheckedOffset)
             -> Result<Option<SegmentReference>, Error> {
        check_field_size!{buffer from; to};
        Ok(None)
    }

}

//\TODO expect some codding of signed ints?
impl<'a> Field<'a> for i8 {

    fn field_size() -> Offset {
        mem::size_of::<Self>() as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, _: Offset) -> i8 {
        buffer[from as usize] as i8
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, _: Offset) {
        buffer[from as usize] = *self as u8;
    }

    fn check(buffer: &'a [u8],
             from: CheckedOffset,
             to: CheckedOffset)
             -> Result<Option<SegmentReference>, Error> {
        check_field_size!{buffer from; to};
        Ok(None)
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

//\TODO should we check `SystemTime` validity in check?
impl<'a> Field<'a> for SystemTime {
    fn field_size() -> Offset {
        (mem::size_of::<u64>() + mem::size_of::<u32>()) as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> SystemTime {
        let secs = LittleEndian::read_u64(&buffer[from as usize .. from as usize + 8]);
        let nanos = LittleEndian::read_u32(&buffer[from as usize + 8..
                                            to as usize]);
        UNIX_EPOCH + Duration::new(secs, nanos)
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        let duration = self.duration_since(UNIX_EPOCH).unwrap();
        let secs = duration.as_secs();
        let nanos = duration.subsec_nanos();
        LittleEndian::write_u64(&mut buffer[from as usize..to as usize - mem::size_of_val(&nanos)],
                                secs);
        LittleEndian::write_u32(&mut buffer[from as usize + mem::size_of_val(&secs)..to as usize],
                                nanos);
    }

    fn check(buffer: &'a [u8],
             from: CheckedOffset,
             to: CheckedOffset)
             -> Result<Option<SegmentReference>, Error> {
        check_field_size!{buffer from; to};
        Ok(None)
    }
}

//\TODO add socketaddr check, for now with only ipv4 
// all possible (>6 bytes long) sequences is a valid addr.
impl<'a> Field<'a> for SocketAddr {
    fn field_size() -> Offset {
        //\FIXME: reserve space for future compatibility
        6
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> SocketAddr {
        let mut octets = [0u8; 4];
        octets.copy_from_slice(&buffer[from as usize..from as usize + 4]);
        let ip = Ipv4Addr::from(octets);
        let port = LittleEndian::read_u16(&buffer[from as usize + 4..to as usize]);
        SocketAddr::V4(SocketAddrV4::new(ip, port))
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        match *self {
            SocketAddr::V4(addr) => {
                buffer[from as usize..to as usize - 2].copy_from_slice(&addr.ip().octets());
            }
            SocketAddr::V6(_) => {
                // FIXME: Supporting Ipv6
                panic!("Ipv6 are currently unsupported")
            }
        }
        LittleEndian::write_u16(&mut buffer[to as usize - 2..to as usize], self.port());
    }

    fn check(buffer: &'a [u8],
             from: CheckedOffset,
             to: CheckedOffset)
             -> Result<Option<SegmentReference>, Error> {
        check_field_size!{buffer from; to};
        Ok(None)
    }
}
