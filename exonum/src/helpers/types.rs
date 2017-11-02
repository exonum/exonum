// Copyright 2017 The Exonum Team
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

//! Common widely used typedefs.

use serde::{Serialize, Serializer, Deserialize, Deserializer};

use std::fmt;

/// Number of milliseconds.
pub type Milliseconds = u64;

/// Blockchain's height (number of blocks).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Height(pub u64);

impl Height {
    /// Returns zero value of the height.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Height;
    ///
    /// let height = Height::zero();
    /// assert_eq!(0, height.0);
    /// ```
    pub fn zero() -> Self {
        Height(0)
    }

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
    pub fn next(&self) -> Self {
        Height(self.0 + 1)
    }

    /// Returns previous value of the height.
    ///
    /// # Panics
    ///
    /// Panics if `self.0` is equal to zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Height;
    ///
    /// let height = Height(10);
    /// let previous_height = height.previous();
    /// assert_eq!(9, previous_height.0);
    /// ```
    pub fn previous(&self) -> Self {
        assert_ne!(0, self.0);
        Height(self.0 - 1)
    }

    /// Increments the height value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Height;
    ///
    /// let mut height = Height::zero();
    /// height.increment();
    /// assert_eq!(1, height.0);
    /// ```
    pub fn increment(&mut self) {
        self.0 += 1;
    }

    /// Decrements the height value.
    ///
    /// # Panics
    ///
    /// Panics if `self.0` is equal to zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Height;
    ///
    /// let mut height = Height(20);
    /// height.decrement();
    /// assert_eq!(19, height.0);
    /// ```
    pub fn decrement(&mut self) {
        assert_ne!(0, self.0);
        self.0 -= 1;
    }
}

/// Consensus round index.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Round(pub u32);

impl Round {
    /// Returns zero value of the round.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Round;
    ///
    /// let round = Round::zero();
    /// assert_eq!(0, round.0);
    /// ```
    pub fn zero() -> Self {
        Round(0)
    }

    /// Returns first value of the round.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Round;
    ///
    /// let round = Round::first();
    /// assert_eq!(1, round.0);
    /// ```
    pub fn first() -> Self {
        Round(1)
    }

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
    pub fn next(&self) -> Self {
        Round(self.0 + 1)
    }

    /// Returns previous value of the round.
    ///
    /// # Panics
    ///
    /// Panics if `self.0` is equal to zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Round;
    ///
    /// let round = Round(10);
    /// let previous_round = round.previous();
    /// assert_eq!(9, previous_round.0);
    /// ```
    pub fn previous(&self) -> Self {
        assert_ne!(0, self.0);
        Round(self.0 - 1)
    }

    /// Increments the round value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Round;
    ///
    /// let mut round = Round::zero();
    /// round.increment();
    /// assert_eq!(1, round.0);
    /// ```
    pub fn increment(&mut self) {
        self.0 += 1;
    }

    /// Decrements the round value.
    ///
    /// # Panics
    ///
    /// Panics if `self.0` is equal to zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Round;
    ///
    /// let mut round = Round(20);
    /// round.decrement();
    /// assert_eq!(19, round.0);
    /// ```
    pub fn decrement(&mut self) {
        assert_ne!(0, self.0);
        self.0 -= 1;
    }

    /// Returns the iterator over rounds in the range from `self` to `to - 1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::Round;
    ///
    /// let round = Round::zero();
    /// let mut iter = round.iter_to(Round(2));
    /// assert_eq!(Some(Round(0)), iter.next());
    /// assert_eq!(Some(Round(1)), iter.next());
    /// assert_eq!(None, iter.next());
    /// ```
    pub fn iter_to(&self, to: Round) -> RoundRangeIter {
        RoundRangeIter {
            next: *self,
            last: to,
        }
    }
}

/// Validators identifier.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ValidatorId(pub u16);

impl ValidatorId {
    /// Returns zero value of the validator id.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::helpers::ValidatorId;
    ///
    /// let id = ValidatorId::zero();
    /// assert_eq!(0, id.0);
    /// ```
    pub fn zero() -> Self {
        ValidatorId(0)
    }
}

impl fmt::Display for Height {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Height> for u64 {
    fn from(val: Height) -> Self {
        val.0
    }
}

impl fmt::Display for Round {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Round> for u32 {
    fn from(val: Round) -> Self {
        val.0
    }
}

impl From<Round> for u64 {
    fn from(val: Round) -> Self {
        u64::from(val.0)
    }
}

impl fmt::Display for ValidatorId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ValidatorId> for u16 {
    fn from(val: ValidatorId) -> Self {
        val.0
    }
}

impl From<ValidatorId> for usize {
    fn from(val: ValidatorId) -> Self {
        val.0 as usize
    }
}

// Serialization/deserialization is implemented manually because TOML round-trip for the tuple
// structs is broken currently. See https://github.com/alexcrichton/toml-rs/issues/194 for details.
impl Serialize for Height {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Height {
    fn deserialize<D>(deserializer: D) -> Result<Height, D::Error>
    where
        D: Deserializer<'de>,
    {

        Ok(Height(u64::deserialize(deserializer)?))
    }
}

/// Iterator over rounds range.
#[derive(Debug)]
pub struct RoundRangeIter {
    next: Round,
    last: Round,
}

// TODO: Add (or replace by) `Step` implementation (ECR-165).
impl Iterator for RoundRangeIter {
    type Item = Round;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next < self.last {
            let res = Some(self.next);
            self.next.increment();
            res
        } else {
            None
        }
    }
}
