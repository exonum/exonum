//! Common widely used typedefs.

/// Blockchain's height (number of blocks).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Height(pub u64);

/// Consensus round index.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Round(pub u32);

/// Validators id.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValidatorId(pub u16);


impl Height {
    /// Returns next value of the height.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Height;
    ///
    /// let height = Height(10);
    /// let next_height = height.next();
    /// assert_eq!(11, next_height.0);
    /// ```
    pub fn next(&self) -> Height {
        Height(self.0 + 1)
    }
}

impl Round {
    /// Returns next value of the round.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Round;
    ///
    /// let round = Round(20);
    /// let next_round = round.next();
    /// assert_eq!(21, next_round.0);
    /// ```
    pub fn next(&self) -> Round {
        Round(self.0 + 1)
    }
}
