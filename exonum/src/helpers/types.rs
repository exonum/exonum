// Copyright 2020 The Exonum Team
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

//! Common widely used type definitions.

use exonum_derive::ObjectHash;
use exonum_merkledb::{impl_binary_key_for_binary_value, BinaryValue};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::{borrow::Cow, fmt, num::ParseIntError, str::FromStr};

/// Number of milliseconds.
pub type Milliseconds = u64;

/// Blockchain height, that is, the number of committed blocks in it.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(ObjectHash)]
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
        Self(0)
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
    pub fn next(self) -> Self {
        Self(self.0 + 1)
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
    pub fn previous(self) -> Self {
        assert_ne!(0, self.0);
        Self(self.0 - 1)
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

impl BinaryValue for Height {
    fn to_bytes(&self) -> Vec<u8> {
        self.0.into_bytes()
    }

    fn from_bytes(value: Cow<'_, [u8]>) -> anyhow::Result<Self> {
        let value = <u64 as BinaryValue>::from_bytes(value)?;
        Ok(Self(value))
    }
}

impl_binary_key_for_binary_value! { Height }

impl fmt::Display for Height {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Height> for u64 {
    fn from(val: Height) -> Self {
        val.0
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
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(u64::deserialize(deserializer)?))
    }
}

impl FromStr for Height {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, ParseIntError> {
        u64::from_str(s).map(Self)
    }
}

/// Consensus round index.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Serialize, Deserialize, ObjectHash)]
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
        Self(0)
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
        Self(1)
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
    pub fn next(self) -> Self {
        Self(self.0 + 1)
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
    pub fn previous(self) -> Self {
        assert_ne!(0, self.0);
        Self(self.0 - 1)
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
    pub fn iter_to(self, to: Self) -> impl Iterator<Item = Self> {
        (self.0..to.0).map(Self)
    }
}

impl BinaryValue for Round {
    fn to_bytes(&self) -> Vec<u8> {
        self.0.into_bytes()
    }

    fn from_bytes(value: Cow<'_, [u8]>) -> anyhow::Result<Self> {
        let value = <u32 as BinaryValue>::from_bytes(value)?;
        Ok(Self(value))
    }
}

impl fmt::Display for Round {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
        Self::from(val.0)
    }
}

/// Validators identifier.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Serialize, Deserialize)]
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
        Self(0)
    }
}

impl BinaryValue for ValidatorId {
    fn to_bytes(&self) -> Vec<u8> {
        self.0.to_bytes()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> anyhow::Result<Self> {
        u16::from_bytes(bytes).map(Self)
    }
}

impl fmt::Display for ValidatorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
        val.0 as Self
    }
}
