// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(unsafe_code)]

use chrono::{DateTime, TimeZone, Utc};
use byteorder::{ByteOrder, LittleEndian};
use uuid::{self, Uuid};

use std::mem;
use std::result::Result as StdResult;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crypto::{Hash, PublicKey, Signature};
use helpers::{Height, Round, ValidatorId};
use super::{CheckedOffset, Error, Offset, Result};

/// Trait for all types that could be a field in `encoding`.
pub trait Field<'a> {
    // TODO: use Read and Cursor (ECR-156)
    // TODO: debug_assert_eq!(to-from == size of Self) (ECR-156)

    /// Field's header size.
    fn field_size() -> Offset;

    /// Read Field from buffer, with given position,
    /// beware of memory unsafety,
    /// you should `check` `Field` before `read`.
    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self;

    /// Write Field to buffer, in given position
    /// `write` doesn't lead to memory unsafety.
    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset);

    /// Checks if data in the buffer could be deserialized.
    /// Returns an index of latest data seen.
    #[allow(unused_variables)]
    fn check(
        buffer: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> ::std::result::Result<CheckedOffset, Error>;
}

/// implement field for all types that has writer and reader functions
///
/// - reader signature is `fn (&[u8]) -> T`
/// - writer signature is `fn (&mut [u8], T)`
#[macro_export]
macro_rules! implement_std_field {
    ($name:ident $fn_read:expr; $fn_write:expr) => (
        impl<'a> Field<'a> for $name {
            fn field_size() -> $crate::encoding::Offset {
                mem::size_of::<$name>() as $crate::encoding::Offset
            }

            unsafe fn read(buffer: &'a [u8],
                           from: $crate::encoding::Offset,
                           to: $crate::encoding::Offset) -> $name {
                $fn_read(&buffer[from as usize..to as usize])
            }

            fn write(&self,
                        buffer: &mut Vec<u8>,
                        from: $crate::encoding::Offset,
                        to: $crate::encoding::Offset) {
                $fn_write(&mut buffer[from as usize..to as usize], *self)
            }

            fn check(_: &'a [u8],
                        from: $crate::encoding::CheckedOffset,
                        to: $crate::encoding::CheckedOffset,
                        latest_segment: CheckedOffset)
            ->  $crate::encoding::Result
            {
                debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());
                Ok(latest_segment)
            }
        }
    )
}

/// Implements `Field` for the tuple struct type definitions that contain simple types.
macro_rules! implement_std_typedef_field {
    ($name:ident ($t:ty) $fn_read:expr; $fn_write:expr) => (
        impl<'a> Field<'a> for $name {
            fn field_size() -> $crate::encoding::Offset {
                mem::size_of::<$t>() as $crate::encoding::Offset
            }

            unsafe fn read(buffer: &'a [u8],
                           from: $crate::encoding::Offset,
                           to: $crate::encoding::Offset) -> $name {
                $name($fn_read(&buffer[from as usize..to as usize]))
            }

            fn write(&self,
                        buffer: &mut Vec<u8>,
                        from: $crate::encoding::Offset,
                        to: $crate::encoding::Offset) {
                $fn_write(&mut buffer[from as usize..to as usize], self.to_owned().into())
            }

            fn check(_: &'a [u8],
                        from: $crate::encoding::CheckedOffset,
                        to: $crate::encoding::CheckedOffset,
                        latest_segment: CheckedOffset)
            ->  $crate::encoding::Result
            {
                debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());
                Ok(latest_segment)
            }
        }
    )
}

/// Implement field helper for all POD types
/// it writes POD type as byte array in place.
///
/// **Beware of platform specific data representation.**
#[macro_export]
macro_rules! implement_pod_as_ref_field {
    ($name:ident) => (
        impl<'a> Field<'a> for &'a $name {
            fn field_size() ->  $crate::encoding::Offset {
                ::std::mem::size_of::<$name>() as $crate::encoding::Offset
            }

            unsafe fn read(buffer: &'a [u8],
                            from: $crate::encoding::Offset,
                            _: $crate::encoding::Offset) -> &'a $name
            {
                ::std::mem::transmute(&buffer[from as usize])
            }

            fn write(&self,
                        buffer: &mut Vec<u8>,
                        from: $crate::encoding::Offset,
                        to: $crate::encoding::Offset)
            {
                let ptr: *const $name = *self as *const $name;
                let slice = unsafe {
                    ::std::slice::from_raw_parts(ptr as * const u8,
                                                        ::std::mem::size_of::<$name>())};
                buffer[from as usize..to as usize].copy_from_slice(slice);
            }

            fn check(_: &'a [u8],
                        from:  $crate::encoding::CheckedOffset,
                        to:  $crate::encoding::CheckedOffset,
                        latest_segment: $crate::encoding::CheckedOffset)
            ->  $crate::encoding::Result
            {
                debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());
                Ok(latest_segment)
            }
        }


    )
}

impl<'a> Field<'a> for bool {
    fn field_size() -> Offset {
        1
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, _: Offset) -> Self {
        buffer[from as usize] == 1
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, _: Offset) {
        buffer[from as usize] = if *self { 1 } else { 0 }
    }

    fn check(
        buffer: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());

        let from: Offset = from.unchecked_offset();
        if buffer[from as usize] != 0 && buffer[from as usize] != 1 {
            Err(Error::IncorrectBoolean {
                position: from,
                value: buffer[from as usize],
            })
        } else {
            Ok(latest_segment)
        }
    }
}

impl<'a> Field<'a> for u8 {
    fn field_size() -> Offset {
        mem::size_of::<Self>() as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, _: Offset) -> Self {
        buffer[from as usize]
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, _: Offset) {
        buffer[from as usize] = *self;
    }

    fn check(
        _: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());
        Ok(latest_segment)
    }
}

// TODO expect some codding of signed integers (ECR-156) ?
impl<'a> Field<'a> for i8 {
    fn field_size() -> Offset {
        mem::size_of::<Self>() as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, _: Offset) -> Self {
        buffer[from as usize] as i8
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, _: Offset) {
        buffer[from as usize] = *self as u8;
    }

    fn check(
        _: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());
        Ok(latest_segment)
    }
}

implement_std_field!{u16 LittleEndian::read_u16; LittleEndian::write_u16}
implement_std_field!{i16 LittleEndian::read_i16; LittleEndian::write_i16}
implement_std_field!{u32 LittleEndian::read_u32; LittleEndian::write_u32}
implement_std_field!{i32 LittleEndian::read_i32; LittleEndian::write_i32}
implement_std_field!{u64 LittleEndian::read_u64; LittleEndian::write_u64}
implement_std_field!{i64 LittleEndian::read_i64; LittleEndian::write_i64}

implement_std_typedef_field!{Height(u64) LittleEndian::read_u64; LittleEndian::write_u64}
implement_std_typedef_field!{Round(u32) LittleEndian::read_u32; LittleEndian::write_u32}
implement_std_typedef_field!{ValidatorId(u16) LittleEndian::read_u16; LittleEndian::write_u16}

implement_pod_as_ref_field! {Signature}
implement_pod_as_ref_field! {PublicKey}
implement_pod_as_ref_field! {Hash}

impl<'a> Field<'a> for DateTime<Utc> {
    fn field_size() -> Offset {
        (mem::size_of::<i64>() + mem::size_of::<u32>()) as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self {
        let secs = LittleEndian::read_i64(&buffer[from as usize..from as usize + 8]);
        let nanos = LittleEndian::read_u32(&buffer[from as usize + 8..to as usize]);
        Utc.timestamp(secs, nanos)
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        let secs = self.timestamp();
        let nanos = self.timestamp_subsec_nanos();
        LittleEndian::write_i64(
            &mut buffer[from as usize..to as usize - mem::size_of_val(&nanos)],
            secs,
        );
        LittleEndian::write_u32(
            &mut buffer[from as usize + mem::size_of_val(&secs)..to as usize],
            nanos,
        );
    }

    fn check(
        _: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());
        Ok(latest_segment)
    }
}

const HEADER_SIZE: usize = 1;
const PORT_SIZE: usize = 2;

const IPV4_SIZE: usize = 4;
const IPV6_SIZE: usize = 16;

const IPV4_HEADER: u8 = 0;
const IPV6_HEADER: u8 = 1;

// TODO add socketaddr check, for now with only ipv4
// all possible (>6 bytes long) sequences is a valid addr (ECR-156).
impl<'a> Field<'a> for SocketAddr {
    fn field_size() -> Offset {
        // FIXME: reserve space for future compatibility (ECR-156)
        (HEADER_SIZE + IPV6_SIZE + PORT_SIZE) as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self {
        let addr_start = from as usize + HEADER_SIZE;
        let ip = match buffer[from as usize] {
            IPV4_HEADER => {
                let mut octets: [u8; IPV4_SIZE] = mem::uninitialized();
                octets.copy_from_slice(&buffer[addr_start..addr_start + IPV4_SIZE]);
                IpAddr::V4(Ipv4Addr::from(octets))
            }
            IPV6_HEADER => {
                let mut octets: [u8; IPV6_SIZE] = mem::uninitialized();
                octets.copy_from_slice(&buffer[addr_start..addr_start + IPV6_SIZE]);
                IpAddr::V6(Ipv6Addr::from(octets))
            }
            header => panic!("Unknown header `{:X}` for SocketAddr", header),
        };
        let port = LittleEndian::read_u16(&buffer[to as usize - PORT_SIZE..to as usize]);
        SocketAddr::new(ip, port)
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        match *self {
            SocketAddr::V4(ref addr) => {
                buffer[from as usize] = IPV4_HEADER;
                let diff = (IPV4_SIZE as isize - IPV6_SIZE as isize).abs() as usize;
                buffer[from as usize + HEADER_SIZE..to as usize - diff - PORT_SIZE]
                    .copy_from_slice(&addr.ip().octets());
            }
            SocketAddr::V6(ref addr) => {
                buffer[from as usize] = IPV6_HEADER;
                buffer[from as usize + HEADER_SIZE..to as usize - PORT_SIZE]
                    .copy_from_slice(&addr.ip().octets());
            }
        }
        LittleEndian::write_u16(
            &mut buffer[to as usize - PORT_SIZE..to as usize],
            self.port(),
        );
    }

    fn check(
        buffer: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());
        let from_offset = from.unchecked_offset();
        if buffer[from_offset as usize] != IPV4_HEADER
            && buffer[from_offset as usize] != IPV6_HEADER
        {
            Err(Error::IncorrectSocketAddrHeader {
                position: from.unchecked_offset(),
                value: buffer[from_offset as usize],
            })
        } else {
            Ok(latest_segment)
        }
    }
}

impl<'a> Field<'a> for Uuid {
    fn field_size() -> Offset {
        16
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self {
        try_read_uuid(buffer, from, to).unwrap()
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        buffer[from as usize..to as usize].copy_from_slice(self.as_bytes());
    }

    fn check(
        buffer: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());
        match try_read_uuid(buffer, from.unchecked_offset(), to.unchecked_offset()) {
            Ok(_) => Ok(latest_segment),
            Err(e) => Err(Error::Other(Box::new(e))),
        }
    }
}

fn try_read_uuid(buffer: &[u8], from: Offset, to: Offset) -> StdResult<Uuid, uuid::ParseError> {
    Uuid::from_bytes(&buffer[from as usize..to as usize])
}
