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
        Self::try_from(value).expect("Unexpected non-finite value")
    }

    /// Creates a new `F32` instance with the given `value`. Returns `None` if the given value
    /// isn't finite.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::encoding::F32;
    ///
    /// let val = F32::try_from(1.0);
    /// assert!(val.is_some());
    ///
    /// let val = F32::try_from(f32::NaN);
    /// assert!(val.is_none());
    /// ```
    pub fn try_from(value: f32) -> Option<Self> {
        if value.is_finite() {
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
        Self::try_from(value).expect("Unexpected non-finite value")
    }

    /// Creates a new `F64` instance with the given `value`. Returns `None` if the given value
    /// isn't finite.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::encoding::F64;
    ///
    /// let val = F64::try_from(1.0);
    /// assert!(val.is_some());
    ///
    /// let val = F64::try_from(f64::NaN);
    /// assert!(val.is_none());
    /// ```
    pub fn try_from(value: f64) -> Option<Self> {
        if value.is_finite() {
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
