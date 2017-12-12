/// Wrapper for the `f32` type that restricts non-finite (NaN and Infinity) values.
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct F32 {
    value: f32,
}

impl F32 {
    /// Creates a new `F32` instance with the given `value`.
    ///
    /// # Panics
    ///
    /// Panics if `is_finite()` returns `false` for the given `value`.
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
        assert!(value.is_finite());
        Self { value }
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

/// Wrapper for the `f64` type that restricts non-numeric (NaN and Infinity) values.
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct F64 {
    value: f64,
}

impl F64 {
    /// Creates a new `F64` instance with the given `value`.
    ///
    /// # Panics
    ///
    /// Panics if `is_finite()` returns `false` for the given `value`.
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
        assert!(value.is_finite());
        Self { value }
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
