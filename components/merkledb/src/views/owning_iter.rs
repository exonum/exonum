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

use std::{marker::PhantomPinned, mem, pin::Pin};

use crate::{
    db::Iterator,
    views::{RawAccess, View},
};

#[derive(Debug)]
struct Inner<T: RawAccess> {
    // Safety: Not accessed during iteration.
    view: View<T>,
    iter: Option<Box<dyn Iterator>>,
    _pin: PhantomPinned,
}

impl<T: RawAccess> Inner<T> {
    #[allow(unsafe_code)]
    fn new(view: View<T>, from: &[u8]) -> Pin<Box<Self>> {
        let mut boxed = Box::pin(Self {
            view,
            iter: None,
            _pin: PhantomPinned,
        });

        unsafe {
            // Elongate the iterator lifetime to `'static`.
            //
            // SAFETY:
            // `T` may not have static lifetime (indeed, `T == &Fork` or `T == &dyn Snapshot`
            // are two frequent use cases), but since `Inner` contains `T` as the type param,
            // `iter` is always valid during the lifetime of the `Inner` instance.
            let iter: Box<dyn Iterator> = mem::transmute(boxed.view.iter_bytes(from));

            let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut boxed);
            // SAFETY: The `iter` field is never considered pinned.
            Pin::get_unchecked_mut(mut_ref).iter = Some(iter);
        }
        boxed
    }

    #[allow(unsafe_code, clippy::borrowed_box)]
    fn iter(self: Pin<&mut Self>) -> &mut Box<dyn Iterator> {
        // SAFETY: The `iter` field is never considered pinned.
        let iter = unsafe { &mut self.get_unchecked_mut().iter };
        // `unwrap()` is safe: Once the `Inner` instance is initialized, `iter` is
        // always `Some(_)`.
        iter.as_mut().unwrap()
    }
}

/// Iterator over a view that owns the view together with the iterator.
/// Thus, the iterator lifetime is only limited by the lifetime of access `T`.
#[derive(Debug)]
pub struct OwningIter<T: RawAccess> {
    inner: Pin<Box<Inner<T>>>,
}

impl<T: RawAccess> OwningIter<T> {
    pub fn new(view: View<T>, from: &[u8]) -> Self {
        Self {
            inner: Inner::new(view, from),
        }
    }
}

impl<T: RawAccess> Iterator for OwningIter<T> {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        Pin::as_mut(&mut self.inner).iter().next()
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        Pin::as_mut(&mut self.inner).iter().peek()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, Fork, TemporaryDB};

    use std::{convert::TryInto, iter::Iterator as StdIterator, str};

    #[test]
    fn owning_iter_basics() {
        let db = TemporaryDB::new();
        let mut fork = db.fork();
        let mut view = View::new(&fork, "test");
        view.put(&1_u8, 2_u8);
        view.put(&2_u8, 3_u8);
        view.put(&3_u8, 4_u8);

        {
            let mut iter = OwningIter::new(view, &[]);
            let mut entries = vec![];
            while let Some((k, v)) = iter.next() {
                entries.push((k[0], v[0]));
            }
            assert_eq!(entries, vec![(1, 2), (2, 3), (3, 4)]);
        }

        // Since iter is dropped, the view should be accessible again.
        let mut view = View::new(&fork, "test");
        view.put(&2_u8, 5_u8);
        {
            let mut iter = OwningIter::new(view, &[2]);
            let mut entries = vec![];
            while let Some((k, v)) = iter.next() {
                entries.push((k[0], v[0]));
            }
            assert_eq!(entries, vec![(2, 5), (3, 4)]);
        }

        fork.flush();
        let view = View::new(&fork, "test");
        {
            let mut iter = OwningIter::new(view, &[1]);
            let mut entries = vec![];
            while let Some((k, v)) = iter.next() {
                entries.push((k[0], v[0]));
            }
            assert_eq!(entries, vec![(1, 2), (2, 5), (3, 4)]);
        }

        let mut view = View::new(&fork, "test");
        view.put(&5_u8, 8_u8);
        view.remove(&1_u8);
        {
            let mut iter = OwningIter::new(view, &[1]);
            let mut entries = vec![];
            while let Some((k, v)) = iter.next() {
                entries.push((k[0], v[0]));
            }
            assert_eq!(entries, vec![(2, 5), (3, 4), (5, 8)]);
        }
    }

    #[test]
    #[should_panic(expected = "Multiple mutable borrows")]
    fn concurrent_borrow_with_iter() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut view = View::new(&fork, "test");
        view.put(&1_u8, 2_u8);
        let _iter = OwningIter::new(view, &[]);
        View::new(&fork, "test");
        // ^-- The view is still borrowed within the iterator, so this should panic.
    }

    fn check_keys(mut iter: OwningIter<&Fork>, mut expected_keys: impl StdIterator<Item = u32>) {
        while let Some((k, v)) = iter.next() {
            let k = u32::from_be_bytes(k.try_into().unwrap());
            let v: u32 = str::from_utf8(v).unwrap().parse().unwrap();
            assert_eq!(k, v);
            assert_eq!(Some(k), expected_keys.next());
        }
        assert_eq!(expected_keys.next(), None);
    }

    #[test]
    fn owning_iter_large() {
        let db = TemporaryDB::new();
        let mut fork = db.fork();
        let mut view = View::new(&fork, "test");
        for i in 0_u32..10_000 {
            view.put(&i, i.to_string());
            view.remove(&(i / 3));
        }

        check_keys(OwningIter::new(view, &[]), (10_000_u32 / 3 + 1)..10_000);

        fork.flush();
        let mut view = View::new(&fork, "test");
        for i in 10_000_u32..20_000 {
            view.put(&i, i.to_string());
            view.remove(&(i / 5));
        }
        for i in 0_u32..1000 {
            view.put(&i, i.to_string());
        }

        check_keys(
            OwningIter::new(view, &[]),
            (0_u32..1000).chain(4_000..20_000),
        );
        let view = View::new(&fork, "test");
        check_keys(
            OwningIter::new(view, &6_000_u32.to_be_bytes()),
            6_000_u32..20_000,
        );
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use crate::{Database, TemporaryDB};

    use proptest::{
        collection::vec, prop_assert, prop_assert_eq, prop_oneof, proptest, strategy,
        strategy::Strategy, test_runner::TestCaseResult,
    };

    use std::{
        collections::{BTreeMap, HashSet},
        ops::Bound,
    };

    const ACTIONS_MAX_LEN: usize = 50;
    const VIEW_NAME: &str = "test";

    #[derive(Debug, Clone)]
    enum Action {
        Put { key: Vec<u8>, value: Vec<u8> },
        Remove(Vec<u8>),
        FlushFork,
        MergeFork,
    }

    fn generate_action() -> impl Strategy<Value = Action> {
        prop_oneof![
            4 => (vec(0_u8..4, 1..2), vec(0_u8..4, 1..5))
                .prop_map(|(key, value)| Action::Put { key, value }),
            4 => vec(0_u8..4, 1..2).prop_map(Action::Remove),
            1 => strategy::Just(Action::FlushFork),
            1 => strategy::Just(Action::MergeFork),
        ]
    }

    fn check_view_range(
        access: impl RawAccess,
        reference: &BTreeMap<Vec<u8>, Vec<u8>>,
        from: &[u8],
    ) -> TestCaseResult {
        let view = View::new(access, VIEW_NAME);
        let mut iter = OwningIter::new(view, from);
        let mut reference_iter =
            reference.range::<[u8], _>((Bound::Included(from), Bound::Unbounded));
        while let Some((key, value)) = iter.next() {
            let (expected_key, expected_value) = if let Some((k, v)) = reference_iter.next() {
                (k, v)
            } else {
                prop_assert!(false, "Reference iter ended before the checked one");
                return Ok(()); // unreachable; needed for type inference
            };
            prop_assert_eq!(key, expected_key.as_slice());
            prop_assert_eq!(value, expected_value.as_slice());
        }
        prop_assert_eq!(reference_iter.next(), None);
        Ok(())
    }

    fn check_view(
        access: impl RawAccess + Copy,
        reference: &BTreeMap<Vec<u8>, Vec<u8>>,
    ) -> TestCaseResult {
        // Some fixed bounds.
        check_view_range(access, reference, &[])?;
        check_view_range(access, reference, &[1])?;
        check_view_range(access, reference, &[1, 2])?;
        check_view_range(access, reference, &[255; 8])?; // Guaranteed to be empty

        // Bounds based on the reference.
        let mut keys = HashSet::new();
        let len = reference.len();
        keys.insert(reference.keys().next());
        keys.insert(reference.keys().nth(2));
        keys.insert(reference.keys().nth(len / 4));
        keys.insert(reference.keys().nth(len / 2));
        keys.insert(reference.keys().nth(2 * len / 3));
        keys.insert(reference.keys().rev().next());

        for key in keys.into_iter().filter_map(|key| key) {
            check_view_range(access, reference, key)?;
        }

        Ok(())
    }

    fn apply_actions(db: &TemporaryDB, actions: Vec<Action>) -> TestCaseResult {
        let mut fork = db.fork();
        let mut reference = BTreeMap::new();
        for action in actions {
            match action {
                Action::Put { key, value } => {
                    View::new(&fork, VIEW_NAME).put(&key, value.clone());
                    reference.insert(key, value);
                }
                Action::Remove(key) => {
                    View::new(&fork, VIEW_NAME).remove(&key);
                    reference.remove(&key);
                }
                Action::FlushFork => fork.flush(),
                Action::MergeFork => {
                    let patch = fork.into_patch();
                    check_view(&patch, &reference)?;
                    db.merge(patch).unwrap();
                    check_view(&db.snapshot(), &reference)?;
                    fork = db.fork();
                }
            }
            check_view(&fork, &reference)?;
        }
        Ok(())
    }

    fn clear(db: &TemporaryDB) {
        let fork = db.fork();
        {
            let mut view = View::new(&fork, VIEW_NAME);
            view.clear();
        }
        db.merge(fork.into_patch()).unwrap();
    }

    #[test]
    fn owning_iter() {
        let db = TemporaryDB::new();
        proptest!(|(actions in vec(generate_action(), 1..ACTIONS_MAX_LEN))| {
            apply_actions(&db, actions)?;
            clear(&db);
        });
    }
}
