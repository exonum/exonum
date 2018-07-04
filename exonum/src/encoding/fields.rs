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

use byteorder::{ByteOrder, LittleEndian};
use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use uuid::{self, Uuid};

use std::{
    mem, net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr}, result::Result as StdResult,
};

use super::{CheckedOffset, Error, Offset, Result};
use crypto::{Hash, PublicKey, Signature};
use helpers::{Height, Round, ValidatorId};

const SOCKET_ADDR_HEADER_SIZE: usize = 1;
const PORT_SIZE: usize = 2;

const IPV4_SIZE: usize = 4;
const IPV6_SIZE: usize = 16;
const SIZE_DIFF: usize = IPV6_SIZE - IPV4_SIZE;

const IPV4_HEADER: u8 = 0;
const IPV6_HEADER: u8 = 1;

const DECIMAL_SIZE: usize = 16;

/// Trait for all types that could be a field in `encoding`.
pub trait Field<'a> {
    // TODO: Use Read and Cursor. (ECR-156)
    // TODO: Debug_assert_eq!(to-from == size of Self). (ECR-156)

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
    /// Default implementation simply checks that the length of segment equals field size.
    #[allow(unused_variables)]
    fn check(
        buffer: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());
        Ok(latest_segment)
    }
}

/// Implements the [`Field`] trait for a type that has writer and reader functions.
///
/// - Reader signature is `fn (&[u8]) -> T`.
/// - Writer signature is `fn (&mut [u8], T)`.
///
/// For additional information, refer to the [`encoding`] module documentation.
///
/// [`Field`]: ./encoding/trait.Field.html
/// [`encoding`]: ./encoding/index.html
#[macro_export]
macro_rules! implement_std_field {
    ($name:ident $fn_read:expr; $fn_write:expr) => {
        impl<'a> Field<'a> for $name {
            fn field_size() -> $crate::encoding::Offset {
                mem::size_of::<$name>() as $crate::encoding::Offset
            }

            unsafe fn read(
                buffer: &'a [u8],
                from: $crate::encoding::Offset,
                to: $crate::encoding::Offset,
            ) -> $name {
                $fn_read(&buffer[from as usize..to as usize])
            }

            fn write(
                &self,
                buffer: &mut Vec<u8>,
                from: $crate::encoding::Offset,
                to: $crate::encoding::Offset,
            ) {
                $fn_write(&mut buffer[from as usize..to as usize], *self)
            }
        }
    };
}

/// Implements `Field` for the tuple struct type definitions that contain simple types.
macro_rules! implement_std_typedef_field {
    ($name:ident($t:ty) $fn_read:expr; $fn_write:expr) => {
        impl<'a> Field<'a> for $name {
            fn field_size() -> $crate::encoding::Offset {
                mem::size_of::<$t>() as $crate::encoding::Offset
            }

            unsafe fn read(
                buffer: &'a [u8],
                from: $crate::encoding::Offset,
                to: $crate::encoding::Offset,
            ) -> $name {
                $name($fn_read(&buffer[from as usize..to as usize]))
            }

            fn write(
                &self,
                buffer: &mut Vec<u8>,
                from: $crate::encoding::Offset,
                to: $crate::encoding::Offset,
            ) {
                $fn_write(
                    &mut buffer[from as usize..to as usize],
                    self.to_owned().into(),
                )
            }
        }
    };
}

/// Implements a field helper for a POD type. This macro enables to convert
/// POD type data into a byte array.
///
/// Additionally, this macro implements the
/// [`ExonumJson`] and [`Field`] traits for data of POD type, so that they can
/// be used within persistent data structures in Exonum.
///
/// For additional information, refer to the [`encoding`] module documentation.
///
/// **Note.** Beware of platform specific data representation.
///
/// [`ExonumJson`]: ./encoding/serialize/json/trait.ExonumJson.html
/// [`Field`]: ./encoding/trait.Field.html
/// [`encoding`]: ./encoding/index.html
#[macro_export]
macro_rules! implement_pod_as_ref_field {
    ($name:ident) => {
        impl<'a> Field<'a> for &'a $name {
            fn field_size() -> $crate::encoding::Offset {
                ::std::mem::size_of::<$name>() as $crate::encoding::Offset
            }

            unsafe fn read(
                buffer: &'a [u8],
                from: $crate::encoding::Offset,
                _: $crate::encoding::Offset,
            ) -> &'a $name {
                &*(&buffer[from as usize] as *const u8 as *const $name)
            }

            fn write(
                &self,
                buffer: &mut Vec<u8>,
                from: $crate::encoding::Offset,
                to: $crate::encoding::Offset,
            ) {
                let ptr: *const $name = *self as *const $name;
                let slice = unsafe {
                    ::std::slice::from_raw_parts(ptr as *const u8, ::std::mem::size_of::<$name>())
                };
                buffer[from as usize..to as usize].copy_from_slice(slice);
            }
        }
    };
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
}

// TODO: Expect some codding of signed integers? (ECR-156)
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
        let secs =
            LittleEndian::read_i64(&buffer[from as usize..from as usize + mem::size_of::<i64>()]);
        let nanos =
            LittleEndian::read_u32(&buffer[from as usize + mem::size_of::<i64>()..to as usize]);
        Utc.timestamp(secs, nanos)
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        let secs = self.timestamp();
        let nanos = self.timestamp_subsec_nanos();
        LittleEndian::write_i64(
            &mut buffer[from as usize..from as usize + mem::size_of::<i64>()],
            secs,
        );
        LittleEndian::write_u32(
            &mut buffer[from as usize + mem::size_of::<i64>()..to as usize],
            nanos,
        );
    }
}

fn is_duration_representation_valid(secs: i64, nanos: i32) -> bool {
    // Signs are checked to avoid multiple representations for same duration.
    // Example: 4 s + 4e8 ns = 5 s - 6e8 ns.
    if (secs < 0 && nanos > 0) || (secs > 0 && nanos < 0) {
        return false;
    }

    // Absolute value of nanoseconds must less than 10 ** 9.
    let nanos_per_sec = 1_000_000_000;
    if nanos <= -nanos_per_sec || nanos >= nanos_per_sec {
        return false;
    }

    true
}

impl<'a> Field<'a> for Duration {
    fn field_size() -> Offset {
        (mem::size_of::<i64>() + mem::size_of::<i32>()) as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self {
        let secs =
            LittleEndian::read_i64(&buffer[from as usize..from as usize + mem::size_of::<i64>()]);
        let nanos =
            LittleEndian::read_i32(&buffer[from as usize + mem::size_of::<i64>()..to as usize]);

        // Assuming that buffer was checked and Duration object can be constructed.
        Duration::seconds(secs) + Duration::nanoseconds(i64::from(nanos))
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        let secs = self.num_seconds();
        let nanos_as_duration = *self - Duration::seconds(secs);
        // Since we're working with only nanos, no overflow is expected here.
        let nanos = nanos_as_duration.num_nanoseconds().unwrap() as i32;

        if !is_duration_representation_valid(secs, nanos) {
            error!(
                "Got Duration object with incorrect representation in Field::write: {}s {}ns",
                secs, nanos
            );
        }

        LittleEndian::write_i64(
            &mut buffer[from as usize..from as usize + mem::size_of::<i64>()],
            secs,
        );
        LittleEndian::write_i32(
            &mut buffer[from as usize + mem::size_of::<i64>()..to as usize],
            nanos,
        );
    }

    fn check(
        buffer: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> Result {
        debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());
        let from_unchecked = from.unchecked_offset() as usize;
        let to_unchecked = to.unchecked_offset() as usize;

        let secs =
            LittleEndian::read_i64(&buffer[from_unchecked..from_unchecked + mem::size_of::<i64>()]);
        let nanos =
            LittleEndian::read_i32(&buffer[from_unchecked + mem::size_of::<i64>()..to_unchecked]);

        let max_duration = Duration::max_value();
        let min_duration = Duration::min_value();

        // Duration::seconds() panics if amount of seconds exceeds limits.
        if secs > max_duration.num_seconds() || secs < min_duration.num_seconds() {
            return Err(Error::DurationOverflow);
        }

        if !is_duration_representation_valid(secs, nanos) {
            return Err(Error::IncorrectDuration { secs, nanos });
        }

        // Result will be None in case of overflow.
        let result = Duration::seconds(secs).checked_add(&Duration::nanoseconds(i64::from(nanos)));
        match result {
            Some(_) => Ok(latest_segment),
            None => Err(Error::DurationOverflow),
        }
    }
}

impl<'a> Field<'a> for SocketAddr {
    fn field_size() -> Offset {
        (SOCKET_ADDR_HEADER_SIZE + IPV6_SIZE + PORT_SIZE) as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self {
        let addr_start = from as usize + SOCKET_ADDR_HEADER_SIZE;
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
                buffer
                    [from as usize + SOCKET_ADDR_HEADER_SIZE..to as usize - SIZE_DIFF - PORT_SIZE]
                    .copy_from_slice(&addr.ip().octets());
                // Padding.
                buffer[to as usize - SIZE_DIFF - PORT_SIZE..to as usize - PORT_SIZE]
                    .copy_from_slice(&[0u8; SIZE_DIFF]);
            }
            SocketAddr::V6(ref addr) => {
                buffer[from as usize] = IPV6_HEADER;
                buffer[from as usize + SOCKET_ADDR_HEADER_SIZE..to as usize - PORT_SIZE]
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

        let from_unchecked = from.unchecked_offset() as usize;
        let to_unchecked = to.unchecked_offset() as usize;

        if buffer[from_unchecked] != IPV4_HEADER && buffer[from_unchecked] != IPV6_HEADER {
            return Err(Error::IncorrectSocketAddrHeader {
                position: from.unchecked_offset(),
                value: buffer[from_unchecked],
            });
        }

        if buffer[from_unchecked] == IPV4_HEADER
            && buffer[to_unchecked - SIZE_DIFF - PORT_SIZE..to_unchecked - PORT_SIZE]
                != [0u8; SIZE_DIFF]
        {
            let mut value: [u8; SIZE_DIFF] = unsafe { mem::uninitialized() };
            value.copy_from_slice(&buffer[to_unchecked - SIZE_DIFF..to_unchecked]);
            return Err(Error::IncorrectSocketAddrPadding {
                position: (to_unchecked - SIZE_DIFF) as Offset,
                value,
            });
        }
        Ok(latest_segment)
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

impl<'a> Field<'a> for Decimal {
    fn field_size() -> Offset {
        DECIMAL_SIZE as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self {
        let mut bytes: [u8; DECIMAL_SIZE] = mem::uninitialized();
        bytes.copy_from_slice(&buffer[from as usize..to as usize]);
        Decimal::deserialize(bytes)
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        buffer[from as usize..to as usize].copy_from_slice(&self.serialize());
    }
}
