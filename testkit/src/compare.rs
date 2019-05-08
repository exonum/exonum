// Copyright 2019 The Exonum Team
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

//! Routines for comparison between 2 states.

use exonum_merkledb::Snapshot;

/// Facilitation of comparison between 2 states.
#[derive(Debug)]
pub struct Comparison<T> {
    old: T,
    new: T,
}

impl<T> Comparison<T> {
    /// Creates a comparison between 2 states.
    pub fn new(old: T, new: T) -> Self {
        Comparison { old, new }
    }

    /// Maps this comparison to another type of objects.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_testkit::compare::Comparison;
    /// let comparison = Comparison::new(vec![1, 2, 3], vec![4, 5])
    ///     .map(Vec::len);
    /// // Now, this is a comparison between `3` and `2`
    /// ```
    pub fn map<'a, F, U>(&'a self, f: F) -> Comparison<U>
    where
        F: Fn(&'a T) -> U,
    {
        Comparison::new(f(&self.old), f(&self.new))
    }

    /// Asserts a statement about the older state in this comparison.
    ///
    /// # Panics
    ///
    /// - Panics if the statement does not hold.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_testkit::compare::Comparison;
    /// Comparison::new(vec![1, 2, 3], vec![4, 5])
    ///     .map(Vec::len)
    ///     .assert_before("Array length <= 5", |&len| len <= 5);
    /// ```
    pub fn assert_before<P>(&self, message: &str, predicate: P) -> &Self
    where
        P: Fn(&T) -> bool,
    {
        assert!(
            predicate(&self.old),
            format!("Precondition does not hold: {}", message)
        );
        self
    }

    /// Asserts a statement about the newer state in this comparison.
    ///
    /// # Panics
    ///
    /// - Panics if the statement does not hold.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_testkit::compare::Comparison;
    /// Comparison::new(vec![1, 2, 3], vec![4, 5])
    ///     .assert_after("The second element is greater than first", |v| {
    ///         v[1] > v[0]
    ///     });
    /// ```
    pub fn assert_after<P>(&self, message: &str, predicate: P) -> &Self
    where
        P: Fn(&T) -> bool,
    {
        assert!(
            predicate(&self.new),
            format!("Postcondition does not hold: {}", message)
        );
        self
    }

    /// Asserts a statement about the both states in this comparison.
    ///
    /// # Panics
    ///
    /// - Panics if the statement does not hold.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_testkit::compare::Comparison;
    /// Comparison::new(vec![1, 2, 3], vec![4, 5])
    ///     .map(Vec::len)
    ///     .assert("Less elements after", |&old, &new| old > new);
    /// ```
    pub fn assert<P>(&self, message: &str, predicate: P) -> &Self
    where
        P: Fn(&T, &T) -> bool,
    {
        assert!(
            predicate(&self.old, &self.new),
            format!("Comparison does not hold: {}", message)
        );
        self
    }

    /// Asserts a statement that should hold true for both states in this comparison.
    ///
    /// # Panics
    ///
    /// - Panics if the statement does not hold for either of states.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_testkit::compare::Comparison;
    /// Comparison::new(vec![1, 2, 3], vec![4, 5])
    ///     .map(Vec::len)
    ///     .assert_inv("More than 1 element in array", |&len| len > 1);
    /// ```
    pub fn assert_inv<P>(&self, message: &str, predicate: P) -> &Self
    where
        P: Fn(&T) -> bool,
    {
        assert!(
            predicate(&self.old),
            format!("Invariant does not hold for the older state: {}", message)
        );
        assert!(
            predicate(&self.new),
            format!("Invariant does not hold for the newer state: {}", message)
        );
        self
    }
}

impl<T: PartialEq + ::std::fmt::Debug> Comparison<T> {
    /// Asserts that the states are equal (by the `PartialEq` definition).
    ///
    /// # Panics
    ///
    /// - Panics if the states are not equal.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_testkit::compare::Comparison;
    /// Comparison::new(vec![1, 2, 3], vec![4, 5, 6])
    ///     .map(Vec::len)
    ///     .assert_eq("Array length doesn't change");
    /// ```
    pub fn assert_eq(&self, message: &str) -> &Self {
        assert_eq!(self.old, self.new, "Invariant does not hold: {}", message);
        self
    }

    /// Asserts that the states are equal (by the `PartialEq` definition).
    ///
    /// # Panics
    ///
    /// - Panics if the states are equal.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_testkit::compare::Comparison;
    /// Comparison::new(vec![1, 2, 3], vec![4, 5])
    ///     .map(Vec::len)
    ///     .assert_ne("Array length should change");
    /// ```
    pub fn assert_ne(&self, message: &str) -> &Self {
        assert_ne!(self.old, self.new, "Expected change: {}", message);
        self
    }
}

/// Trait facilitating comparison between 2 `Snapshot`s taken at different times.
///
/// # Examples
///
/// Typical usage involves `map`ping the resulting comparison through the schema:
///
/// ```ignore
/// let mut testkit = ...;
/// let old_snapshot = testkit.snapshot();
/// // Mutate the testkit state somehow...
///
/// testkit.snapshot()
///     .compare(old_snapshot)
///     .map(ServiceSchema::new)
///     .assert("Something about the schema", |old, schema| {
///         // Assertions...
///     });
/// ```
///
/// Here `ServiceSchema` is a public struct defined in a service library that has public `new`
/// method with a signature like `fn<S: AsRef<Snapshot>>(view: S) -> Self`.
pub trait ComparableSnapshot<S> {
    /// Compares this snapshot with an older one.
    fn compare(self, old: S) -> Comparison<Box<dyn Snapshot>>;
}

impl ComparableSnapshot<Box<dyn Snapshot>> for Box<dyn Snapshot> {
    fn compare(self, old: Box<dyn Snapshot>) -> Comparison<Box<dyn Snapshot>> {
        Comparison::new(old, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assertions() {
        let comp = Comparison::new(vec![1, 2, 3], vec![4, 5, 6]);
        comp.assert_before("Array should have length 3", |old| old.len() == 3)
            .assert_after("Array should have length 3", |new| new.len() == 3)
            .assert("Array should be transformed", |old, new| {
                new.iter().enumerate().all(|(i, &x)| x == old[i] + 3)
            })
            .map(Vec::len)
            .assert("Lengths should be the same", |old, new| old == new);
    }

    #[test]
    #[should_panic(expected = "Precondition does not hold: Array should have length 3")]
    fn test_assertion_precondition_failure() {
        let comp = Comparison::new(vec![1, 2], vec![4, 5, 6]);
        comp.assert_before("Array should have length 3", |old| old.len() == 3);
    }

    #[test]
    #[should_panic(expected = "Postcondition does not hold: Array should have length 3")]
    fn test_assertion_postcondition_failure() {
        let comp = Comparison::new(vec![1, 2, 3], vec![4, 5]);
        comp.assert_after("Array should have length 3", |new| new.len() == 3);
    }

    #[test]
    #[should_panic(expected = "Comparison does not hold: Array should be transformed")]
    fn test_assertion_transform_failure() {
        let comp = Comparison::new(vec![1, 2, 3], vec![4, 5, 7]);
        comp.assert("Array should be transformed", |old, new| {
            new.iter().enumerate().all(|(i, &x)| x == old[i] + 3)
        });
    }

    #[test]
    #[should_panic(expected = "Array length more than 1")]
    fn test_assertion_invariant_failure_pre() {
        let comp = Comparison::new(vec![1], vec![2, 3, 4]);
        comp.assert_inv("Array length more than 1", |v| v.len() > 1);
    }

    #[test]
    #[should_panic(expected = "Array length more than 1")]
    fn test_assertion_invariant_failure_post() {
        let comp = Comparison::new(vec![1, 2, 3], vec![4]);
        comp.assert_inv("Array length more than 1", |v| v.len() > 1);
    }
}
