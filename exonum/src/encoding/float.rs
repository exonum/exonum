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

use byteorder::{ByteOrder, LittleEndian};
use serde_json::value::{Number, Value};

use std::{error::Error, mem};

use super::{Error as EncodingError, Result as EncodingResult};
use encoding::{
    serialize::json::{ExonumJson, ExonumJsonDeserialize}, serialize::WriteBufferWrapper,
    CheckedOffset, Field, Offset,
};

/// Wrapper for the `f32` type that restricts non-finite
/// (NaN, Infinity, negative zero and subnormal) values.
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct F32 {
    value: f32,
}

impl F32 {
    /// Creates a new `F32` instance with the given `value`.
    ///
    /// # Panics
    ///
    /// Panics if given value isn't normal (either `NaN`, `Infinity`, negative zero or `SubNormal`).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::encoding::F32;
    ///
    /// let val = F32::new(1.0);
    /// assert_eq!(val.get(), 1.0);
    /// ```
    pub fn new(value: f32) -> Self {
        Self::try_from(value).expect("Unexpected non-finite value")
    }

    /// Creates a new `F32` instance with the given `value`. Returns `None` if the given value
    /// isn't normal (either `NaN`, `Infinity`, negative zero or `SubNormal`).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::encoding::F32;
    /// use std::f32;
    ///
    /// let val = F32::try_from(1.0);
    /// assert!(val.is_some());
    ///
    /// let val = F32::try_from(f32::NAN);
    /// assert!(val.is_none());
    /// ```
    pub fn try_from(value: f32) -> Option<Self> {
        if value.is_normal() || (value == 0.0 && value.signum() == 1.0) {
            Some(Self { value })
        } else {
            None
        }
    }

    /// Returns value contained in this wrapper.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::encoding::F32;
    ///
    /// let wrapper = F32::new(1.0);
    /// let value = wrapper.get();
    /// ```
    pub fn get(&self) -> f32 {
        self.value
    }
}

/// Wrapper for the `f64` type that restricts non-finite
/// (NaN, Infinity, negative zero and subnormal) values.
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct F64 {
    value: f64,
}

impl F64 {
    /// Creates a new `F64` instance with the given `value`.
    ///
    /// # Panics
    ///
    /// Panics if given value isn't normal (either `NaN`, `Infinity`, negative zero or `SubNormal`).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::encoding::F64;
    ///
    /// let val = F64::new(1.0);
    /// assert_eq!(val.get(), 1.0);
    /// ```
    pub fn new(value: f64) -> Self {
        Self::try_from(value).expect("Unexpected non-finite value")
    }

    /// Creates a new `F64` instance with the given `value`. Returns `None` if the given value
    /// isn't normal (either `NaN`, `Infinity`, negative zero or `SubNormal`).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::encoding::F64;
    /// use std::f64;
    ///
    /// let val = F64::try_from(1.0);
    /// assert!(val.is_some());
    ///
    /// let val = F64::try_from(f64::NAN);
    /// assert!(val.is_none());
    /// ```
    pub fn try_from(value: f64) -> Option<Self> {
        if value.is_normal() || (value == 0.0 && value.signum() == 1.0) {
            Some(Self { value })
        } else {
            None
        }
    }

    /// Returns value contained in this wrapper.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::encoding::F64;
    ///
    /// let wrapper = F64::new(1.0);
    /// let value = wrapper.get();
    /// ```
    pub fn get(&self) -> f64 {
        self.value
    }
}

impl<'a> Field<'a> for F32 {
    fn field_size() -> Offset {
        mem::size_of::<Self>() as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self {
        Self::new(LittleEndian::read_f32(&buffer[from as usize..to as usize]))
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        LittleEndian::write_f32(&mut buffer[from as usize..to as usize], self.get());
    }

    fn check(
        buffer: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> EncodingResult {
        debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());

        let from = from.unchecked_offset();
        let to = to.unchecked_offset();

        let value = LittleEndian::read_f32(&buffer[from as usize..to as usize]);
        match Self::try_from(value) {
            Some(_) => Ok(latest_segment),
            None => Err(EncodingError::UnsupportedFloat {
                position: from,
                value: f64::from(value),
            }),
        }
    }
}

impl<'a> Field<'a> for F64 {
    fn field_size() -> Offset {
        mem::size_of::<Self>() as Offset
    }

    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self {
        Self::new(LittleEndian::read_f64(&buffer[from as usize..to as usize]))
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        LittleEndian::write_f64(&mut buffer[from as usize..to as usize], self.get());
    }

    fn check(
        buffer: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> EncodingResult {
        debug_assert_eq!((to - from)?.unchecked_offset(), Self::field_size());

        let from = from.unchecked_offset();
        let to = to.unchecked_offset();

        let value = LittleEndian::read_f64(&buffer[from as usize..to as usize]);
        match Self::try_from(value) {
            Some(_) => Ok(latest_segment),
            None => Err(EncodingError::UnsupportedFloat {
                position: from,
                value,
            }),
        }
    }
}

impl ExonumJson for F32 {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let number = value.as_f64().ok_or("Can't cast json as float")?;
        buffer.write(
            from,
            to,
            Self::try_from(number as f32).ok_or("Invalid float value in json")?,
        );
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        Ok(Value::Number(
            Number::from_f64(f64::from(self.get())).ok_or("Can't cast float as json")?
        ))
    }
}

impl ExonumJsonDeserialize for F32 {
    fn deserialize(value: &Value) -> Result<Self, Box<Error>> {
        let number = value.as_f64().ok_or("Can't cast json as float")?;
        Ok(Self::try_from(number as f32).ok_or("Invalid float value in json")?)
    }
}

impl ExonumJson for F64 {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let number = value.as_f64().ok_or("Can't cast json as float")?;
        buffer.write(
            from,
            to,
            Self::try_from(number).ok_or("Invalid float value in json")?,
        );
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        Ok(Value::Number(
            Number::from_f64(self.get()).ok_or("Can't cast float as json")?
        ))
    }
}

impl ExonumJsonDeserialize for F64 {
    fn deserialize(value: &Value) -> Result<Self, Box<Error>> {
        let number = value.as_f64().ok_or("Can't cast json as float")?;
        Ok(Self::try_from(number).ok_or("Invalid float value in json")?)
    }
}

#[cfg(test)]
mod tests {
    use super::{F32, F64};
    use byteorder::{ByteOrder, LittleEndian};
    use encoding::fields::Field;
    use encoding::Offset;
    use std::num::FpCategory;
    use std::panic;
    use std::{f32, f64};

    fn validate_constructor<T, V, C: Fn(V) -> T>(
        constructor: C,
        value: V,
        buffer: &[u8],
        header_size: Offset,
    ) -> bool
    where
        C: panic::RefUnwindSafe,
        V: panic::UnwindSafe,
        T: for<'r> Field<'r> + PartialEq + ::std::fmt::Debug,
    {
        let constructor_result = panic::catch_unwind(|| constructor(value));
        let check_result =
            <T as Field>::check(&buffer, 0.into(), header_size.into(), header_size.into());
        if constructor_result.is_err() && check_result.is_err() {
            return false;
        } else if constructor_result.is_ok() && check_result.is_ok() {
            let constructed = constructor_result.unwrap();
            let read = unsafe { <T as Field>::read(&buffer, 0, header_size) };
            assert_eq!(constructed, read);
            return true;
        } else {
            panic!("{:?} != {:?}", constructor_result, check_result);
        }
    }

    #[test]
    fn test_f32_encoding() {
        let sub: f32 = 1.1754942e-38;
        assert_eq!(sub.classify(), FpCategory::Subnormal);
        let valid_data = vec![0f32, 3.14, -1.0, 1.0, f32::MAX, f32::MIN];
        let invalid_data = vec![-0.0f32, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, sub];
        let mut buf = vec![0; 4];
        for value in valid_data {
            LittleEndian::write_f32(&mut buf, value);
            assert!(validate_constructor(|v| F32::new(v), value, &buf, 4));
        }
        for value in invalid_data {
            LittleEndian::write_f32(&mut buf, value);
            assert!(!validate_constructor(|v| F32::new(v), value, &buf, 4));
        }
    }

    #[test]
    fn test_f64_encoding() {
        let sub: f64 = 1.1754942e-315;
        assert_eq!(sub.classify(), FpCategory::Subnormal);
        let valid_data = vec![0f64, 3.14, -1.0, 1.0, f64::MAX, f64::MIN];
        let invalid_data = vec![-0.0f64, f64::INFINITY, f64::NEG_INFINITY, f64::NAN, sub];
        let mut buf = vec![0; 8];
        for value in valid_data {
            LittleEndian::write_f64(&mut buf, value);
            assert!(validate_constructor(|v| F64::new(v), value, &buf, 8));
        }
        for value in invalid_data {
            LittleEndian::write_f64(&mut buf, value);
            assert!(!validate_constructor(|v| F64::new(v), value, &buf, 8));
        }
    }

    #[test]
    #[allow(dead_code)]
    fn test_f32_struct() {
        encoding_struct! {
            struct Msg {
                single_float: F32,
                vec: Vec<F32>,
            }
        }
        let test_vec = vec![F32::new(0.0), F32::new(3.14), F32::new(5.82)];

        let msg = Msg::new(F32::new(0.0), test_vec.clone());
        assert_eq!(msg.single_float().get(), 0.0);
        assert_eq!(msg.vec(), test_vec);
    }

    #[test]
    #[allow(dead_code)]
    fn test_f64_struct() {
        encoding_struct! {
            struct Msg {
                single_float: F64,
                vec: Vec<F64>,
            }
        }

        let test_vec = vec![F64::new(0.0), F64::new(3.14), F64::new(5.82)];

        let msg = Msg::new(F64::new(0.0), test_vec.clone());
        assert_eq!(msg.single_float().get(), 0.0);
        assert_eq!(msg.vec(), test_vec);
    }
}
