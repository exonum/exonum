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

use assert_matches::assert_matches;
use url::form_urlencoded::byte_serialize;

use std::{num::NonZeroU64, panic, rc::Rc};

use crate::{
    access::CopyAccessExt,
    db,
    validation::is_valid_identifier,
    views::{IndexAddress, IndexType, RawAccess, View, ViewWithMetadata},
    Database, DbOptions, Fork, ListIndex, MapIndex, ResolvedAddress, RocksDB, TemporaryDB,
};

const IDX_NAME: &str = "idx_name";
const PREFIXED_IDX: (&str, u64) = ("idx", 42);

// Conversion to simplify `ResolvedAddress` instantiation for tests. This conversion
// is not used in the main code, so it's intentionally placed here.
impl From<(&str, u64)> for ResolvedAddress {
    fn from((name, id): (&str, u64)) -> Self {
        Self::new(name, NonZeroU64::new(id))
    }
}

fn assert_iter<T: RawAccess>(view: &View<T>, from: u8, assumed: &[(u8, u8)]) {
    let mut iter = view.iter_bytes(&[from]);
    let mut values = Vec::new();
    while let Some((k, v)) = iter.next() {
        values.push((k[0], v[0]));
    }

    assert_eq!(values, assumed);
}

fn assert_initial_state<T: RawAccess>(view: &View<T>) {
    assert_eq!(view.get_bytes(&[1]), Some(vec![1]));
    assert_eq!(view.get_bytes(&[2]), Some(vec![2]));
    assert_eq!(view.get_bytes(&[3]), Some(vec![3]));
    assert_eq!(view.get_bytes(&[4]), None);
}

fn test_changelog<T, I>(db: &T, address: I)
where
    T: Database,
    I: Into<ResolvedAddress> + Copy,
{
    let mut fork = db.fork();
    {
        let mut view = View::new(&fork, address);
        view.put(&vec![1], vec![1]);
        view.put(&vec![2], vec![2]);
        view.put(&vec![3], vec![3]);

        assert_initial_state(&view);
    }
    fork.flush();

    {
        let mut view = View::new(&fork, address);
        assert_initial_state(&view);

        view.put(&vec![1], vec![10]);
        view.put(&vec![4], vec![40]);
        view.remove(&vec![2]);

        assert_eq!(view.get_bytes(&[1]), Some(vec![10]));
        assert_eq!(view.get_bytes(&[2]), None);
        assert_eq!(view.get_bytes(&[3]), Some(vec![3]));
        assert_eq!(view.get_bytes(&[4]), Some(vec![40]));
    }
    fork.rollback();

    {
        let view = View::new(&fork, address);
        assert_initial_state(&view);
    }
    fork.flush();

    {
        let mut view = View::new(&fork, address);
        view.put(&vec![4], vec![40]);
        view.put(&vec![4], vec![41]);
        view.remove(&vec![2]);
        view.put(&vec![2], vec![20]);

        assert_eq!(view.get_bytes(&[1]), Some(vec![1]));
        assert_eq!(view.get_bytes(&[2]), Some(vec![20]));
        assert_eq!(view.get_bytes(&[3]), Some(vec![3]));
        assert_eq!(view.get_bytes(&[4]), Some(vec![41]));
    }
    fork.rollback();

    {
        let view = View::new(&fork, address);
        assert_initial_state(&view);
        assert_eq!(view.get_bytes(&[4]), None);
    }

    View::new(&fork, address).put(&vec![2], vec![20]);
    fork.flush();
    View::new(&fork, address).put(&vec![3], vec![30]);
    fork.rollback();

    let view = View::new(&fork, address);
    assert_eq!(view.get_bytes(&[1]), Some(vec![1]));
    assert_eq!(view.get_bytes(&[2]), Some(vec![20]));
    assert_eq!(view.get_bytes(&[3]), Some(vec![3]));
    assert_eq!(view.get_bytes(&[4]), None);
}

fn _views_in_same_family<T: Database>(db: &T) {
    const IDX_1: (&str, u64) = ("foo", 23);
    const IDX_2: (&str, u64) = ("foo", 42);

    let mut fork = db.fork();
    {
        let mut view1 = View::new(&fork, IDX_1);
        let mut view2 = View::new(&fork, IDX_2);

        view1.put(&vec![1], vec![10]);
        view1.put(&vec![2], vec![20]);

        assert_eq!(view1.get_bytes(&[1]), Some(vec![10]));
        assert_eq!(view2.get_bytes(&[1]), None);
        assert_iter(&view1, 1, &[(1, 10), (2, 20)]);
        assert_iter(&view2, 1, &[]);

        view2.put(&vec![1], vec![1]);
        view2.put(&vec![1], vec![2]);
        view2.put(&vec![2], vec![4]);
        view2.put(&vec![0], vec![0, 1, 2, 3]);

        assert_eq!(view1.get_bytes(&[1]), Some(vec![10]));
        assert_eq!(view2.get_bytes(&[1]), Some(vec![2]));
        assert_iter(&view1, 1, &[(1, 10), (2, 20)]);
        assert_iter(&view2, 1, &[(1, 2), (2, 4)]);
    }
    fork.flush();

    {
        let mut view1 = View::new(&fork, IDX_1);
        let view2 = View::new(&fork, IDX_2);

        assert_iter(&view1, 1, &[(1, 10), (2, 20)]);
        assert_iter(&view2, 1, &[(1, 2), (2, 4)]);

        view1.put(&vec![2], vec![30]);
        view1.put(&vec![3], vec![40]);
        view1.put(&vec![0], vec![0]);

        assert_iter(&view1, 1, &[(1, 10), (2, 30), (3, 40)]);
        assert_iter(&view2, 1, &[(1, 2), (2, 4)]);

        view1.remove(&vec![0]);
    }
    db.merge(fork.into_patch()).unwrap();

    let snapshot = db.snapshot();
    let view1 = View::new(&snapshot, IDX_1);
    let view2 = View::new(&snapshot, IDX_2);

    assert_iter(&view1, 0, &[(1, 10), (2, 30), (3, 40)]);
    assert_iter(&view2, 0, &[(0, 0), (1, 2), (2, 4)]);
}

fn test_two_mutable_borrows<T, I>(db: &T, address: I)
where
    T: Database,
    I: Into<ResolvedAddress> + Copy,
{
    let fork = db.fork();

    let view1 = View::new(&fork, address);
    let view2 = View::new(&fork, address);
    assert_eq!(view1.get_bytes(&[0]), None);
    assert_eq!(view2.get_bytes(&[0]), None);
}

fn test_mutable_and_immutable_borrows<T, I>(db: &T, address: I)
where
    T: Database,
    I: Into<ResolvedAddress> + Copy,
{
    let fork = db.fork();

    let view1 = View::new(&fork, address);
    let view2 = View::new(fork.readonly(), address);
    assert_eq!(view1.get_bytes(&[0]), None);
    assert_eq!(view2.get_bytes(&[0]), None);
}

fn test_clear_view<T, I>(db: &T, address: I)
where
    T: Database,
    I: Into<ResolvedAddress> + Copy,
{
    let fork = db.fork();
    {
        let mut view = View::new(&fork, address);
        view.put(&vec![1], vec![1, 2]);
        view.put(&vec![2], vec![3, 4]);
        view.clear();

        assert_eq!(view.get_bytes(&[1]), None);
        assert_iter(&view, 0, &[]);
        assert_iter(&view, 1, &[]);

        view.put(&vec![1], vec![5]);
        view.put(&vec![3], vec![6]);
        assert_eq!(view.get_bytes(&[1]), Some(vec![5]));
        assert_iter(&view, 0, &[(1, 5), (3, 6)]);
        assert_iter(&view, 2, &[(3, 6)]);
    }
    db.merge(fork.into_patch()).unwrap();

    {
        let snapshot = db.snapshot();
        let view = View::new(&snapshot, address);

        assert_eq!(view.get_bytes(&[1]), Some(vec![5]));
        assert_iter(&view, 0, &[(1, 5), (3, 6)]);
        assert_iter(&view, 2, &[(3, 6)]);
    }

    let fork = db.fork();
    {
        let mut view = View::new(&fork, address);
        view.put(&vec![1], vec![3, 4]);

        view.clear();
        view.put(&vec![4], vec![0]);
        view.put(&vec![3], vec![0]);

        assert_eq!(view.get_bytes(&[1]), None);
        assert_eq!(view.get_bytes(&[3]), Some(vec![0]));
        assert_iter(&view, 0, &[(3, 0), (4, 0)]);
        assert_iter(&view, 4, &[(4, 0)]);
    }
    {
        let view = View::new(&fork, address);

        assert_eq!(view.get_bytes(&[1]), None);
        assert_eq!(view.get_bytes(&[3]), Some(vec![0]));
        assert_iter(&view, 0, &[(3, 0), (4, 0)]);
        assert_iter(&view, 4, &[(4, 0)]);
    }

    db.merge(fork.into_patch()).unwrap();
    let snapshot = db.snapshot();
    let view = View::new(&snapshot, address);
    assert_iter(&view, 0, &[(3, 0), (4, 0)]);
    assert_iter(&view, 4, &[(4, 0)]);
}

fn test_fork_iter<T, I>(db: &T, address: I)
where
    T: Database,
    I: Into<ResolvedAddress> + Copy,
{
    let fork = db.fork();
    {
        let view = View::new(&fork, address);
        let mut view = view;
        view.put(&vec![10], vec![10]);
        view.put(&vec![20], vec![20]);
        view.put(&vec![30], vec![30]);
        assert!(view.contains_raw_key(&[10]));
    }
    db.merge(fork.into_patch()).unwrap();

    let fork = db.fork();
    let mut view = View::new(&fork, address);
    assert!(view.contains_raw_key(&[10]));

    // Stored
    assert_iter(&view, 0, &[(10, 10), (20, 20), (30, 30)]);
    assert_iter(&view, 5, &[(10, 10), (20, 20), (30, 30)]);
    assert_iter(&view, 10, &[(10, 10), (20, 20), (30, 30)]);
    assert_iter(&view, 11, &[(20, 20), (30, 30)]);
    assert_iter(&view, 31, &[]);

    // Inserted
    view.put(&vec![5], vec![5]);
    assert_iter(&view, 0, &[(5, 5), (10, 10), (20, 20), (30, 30)]);
    view.put(&vec![25], vec![25]);
    assert_iter(&view, 0, &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30)]);
    view.put(&vec![35], vec![35]);
    assert_iter(
        &view,
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30), (35, 35)],
    );

    // Double inserted
    view.put(&vec![25], vec![23]);
    assert_iter(
        &view,
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 23), (30, 30), (35, 35)],
    );
    view.put(&vec![26], vec![26]);
    assert_iter(
        &view,
        0,
        &[
            (5, 5),
            (10, 10),
            (20, 20),
            (25, 23),
            (26, 26),
            (30, 30),
            (35, 35),
        ],
    );

    // Replaced
    let fork = db.fork();
    let mut view = View::new(&fork, address);

    view.put(&vec![10], vec![11]);
    assert_iter(&view, 0, &[(10, 11), (20, 20), (30, 30)]);
    view.put(&vec![30], vec![31]);
    assert_iter(&view, 0, &[(10, 11), (20, 20), (30, 31)]);

    // Deleted
    let fork = db.fork();
    let mut view = View::new(&fork, address);

    view.remove(&vec![20]);
    assert_iter(&view, 0, &[(10, 10), (30, 30)]);
    view.remove(&vec![10]);
    assert_iter(&view, 0, &[(30, 30)]);
    view.put(&vec![10], vec![11]);
    assert_iter(&view, 0, &[(10, 11), (30, 30)]);
    view.remove(&vec![10]);
    assert_iter(&view, 0, &[(30, 30)]);

    // MissDeleted
    let fork = db.fork();
    let mut view = View::new(&fork, address);

    view.remove(&vec![5]);
    assert_iter(&view, 0, &[(10, 10), (20, 20), (30, 30)]);
    view.remove(&vec![15]);
    assert_iter(&view, 0, &[(10, 10), (20, 20), (30, 30)]);
    view.remove(&vec![35]);
    assert_iter(&view, 0, &[(10, 10), (20, 20), (30, 30)]);
}

#[test]
fn test_database_check_correct_version() {
    let db = TemporaryDB::default();
    let snapshot = db.snapshot();

    let view = View::new(&snapshot, ResolvedAddress::system(db::DB_METADATA));
    let version: u8 = view.get(db::VERSION_NAME).unwrap();
    assert_eq!(version, db::DB_VERSION);
}

#[test]
#[should_panic(expected = "actual 2, expected 0")]
fn test_database_check_incorrect_version() {
    let dir = tempfile::TempDir::new().unwrap();
    let opts = DbOptions::default();
    // Writes different version to metadata.
    {
        let db = RocksDB::open(&dir, &opts).unwrap();
        let fork = db.fork();
        {
            let mut view = View::new(&fork, ResolvedAddress::system(db::DB_METADATA));
            view.put(db::VERSION_NAME, 2_u8);
        }
        db.merge(fork.into_patch()).unwrap();
    }
    // Tries to open modified database.
    RocksDB::open(&dir, &opts).unwrap();
}

#[test]
fn fork_iter() {
    test_fork_iter(&TemporaryDB::new(), IDX_NAME);
}

#[test]
fn fork_iter_prefixed() {
    test_fork_iter(&TemporaryDB::new(), PREFIXED_IDX);
}

#[test]
fn changelog() {
    test_changelog(&TemporaryDB::new(), IDX_NAME);
}

#[test]
fn changelog_prefixed() {
    test_changelog(&TemporaryDB::new(), PREFIXED_IDX);
}

#[test]
fn multiple_views() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        // Writing to multiple views at the same time
        let mut view = View::new(&fork, IDX_NAME);
        let mut prefixed_view = View::new(&fork, PREFIXED_IDX);

        view.put(&vec![1], vec![10]);
        prefixed_view.put(&vec![1], vec![30]);
        view.put(&vec![2], vec![20]);
        view.put(&vec![3], vec![30]);
        prefixed_view.put(&vec![3], vec![40]);
        prefixed_view.put(&vec![5], vec![50]);

        assert_eq!(view.get_bytes(&[1]), Some(vec![10]));
        assert_eq!(prefixed_view.get_bytes(&[1]), Some(vec![30]));
        assert_iter(&view, 0, &[(1, 10), (2, 20), (3, 30)]);
        assert_iter(&prefixed_view, 0, &[(1, 30), (3, 40), (5, 50)]);
    }

    db.merge(fork.into_patch()).unwrap();
    {
        // Reading from a snapshot
        let snapshot = db.snapshot();
        let view = View::new(&snapshot, IDX_NAME);
        let prefixed_view = View::new(&snapshot, PREFIXED_IDX);

        assert_iter(&view, 0, &[(1, 10), (2, 20), (3, 30)]);
        assert_iter(&prefixed_view, 0, &[(1, 30), (3, 40), (5, 50)]);
    }

    let fork = db.fork();
    {
        // Reading from one view and writing to other
        let view = View::new(&fork, IDX_NAME);
        let mut prefixed_view = View::new(&fork, PREFIXED_IDX);

        assert_iter(&view, 0, &[(1, 10), (2, 20), (3, 30)]);
        assert_iter(&prefixed_view, 0, &[(1, 30), (3, 40), (5, 50)]);

        prefixed_view.remove(&vec![3]);
        prefixed_view.put(&vec![2], vec![60]);
        assert_iter(&view, 0, &[(1, 10), (2, 20), (3, 30)]);
        assert_iter(&prefixed_view, 0, &[(1, 30), (2, 60), (5, 50)]);
    }
}

#[test]
fn multiple_indexes() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut list: ListIndex<_, u32> = fork.get_list(IDX_NAME);
        let mut map = fork.get_map(("idx", &3));

        for i in 0..10 {
            list.push(i);
            if i % 2 == 0 {
                map.put(&i, "??".to_owned());
            }
        }
    }
    db.merge(fork.into_patch()).unwrap();

    let snapshot = db.snapshot();
    let list: ListIndex<_, u32> = snapshot.get_list(IDX_NAME);
    let map: MapIndex<_, u32, String> = snapshot.get_map(("idx", &3));
    assert_eq!(list.len(), 10);
    assert!(map.values().all(|val| val == "??"));

    let fork = db.fork();
    let list: ListIndex<_, u32> = fork.get_list(IDX_NAME);
    let mut map = fork.get_map(("idx", &3));
    for item in &list {
        map.put(&item, item.to_string());
    }

    assert_eq!(map.values().count(), 10);
    assert!(map.iter_from(&3).all(|(k, v)| k < 10 && v == k.to_string()));
}

#[test]
fn views_in_same_family() {
    const IDX_1: (&str, u64) = ("foo", 23);
    const IDX_2: (&str, u64) = ("foo", 42);

    let db = TemporaryDB::new();

    let mut fork = db.fork();
    {
        let mut view1 = View::new(&fork, IDX_1);
        let mut view2 = View::new(&fork, IDX_2);

        view1.put(&vec![1], vec![10]);
        view1.put(&vec![2], vec![20]);

        assert_eq!(view1.get_bytes(&[1]), Some(vec![10]));
        assert_eq!(view2.get_bytes(&[1]), None);
        assert_iter(&view1, 1, &[(1, 10), (2, 20)]);
        assert_iter(&view2, 1, &[]);

        view2.put(&vec![1], vec![1]);
        view2.put(&vec![1], vec![2]);
        view2.put(&vec![2], vec![4]);
        view2.put(&vec![0], vec![0, 1, 2, 3]);

        assert_eq!(view1.get_bytes(&[1]), Some(vec![10]));
        assert_eq!(view2.get_bytes(&[1]), Some(vec![2]));
        assert_iter(&view1, 1, &[(1, 10), (2, 20)]);
        assert_iter(&view2, 1, &[(1, 2), (2, 4)]);
    }
    fork.flush();

    {
        let mut view1 = View::new(&fork, IDX_1);
        let view2 = View::new(&fork, IDX_2);

        assert_iter(&view1, 1, &[(1, 10), (2, 20)]);
        assert_iter(&view2, 1, &[(1, 2), (2, 4)]);

        view1.put(&vec![2], vec![30]);
        view1.put(&vec![3], vec![40]);
        view1.put(&vec![0], vec![0]);

        assert_iter(&view1, 1, &[(1, 10), (2, 30), (3, 40)]);
        assert_iter(&view2, 1, &[(1, 2), (2, 4)]);

        view1.remove(&vec![0]);
    }
    db.merge(fork.into_patch()).unwrap();

    let snapshot = db.snapshot();
    let view1 = View::new(&snapshot, IDX_1);
    let view2 = View::new(&snapshot, IDX_2);

    assert_iter(&view1, 0, &[(1, 10), (2, 30), (3, 40)]);
    assert_iter(&view2, 0, &[(0, 0), (1, 2), (2, 4)]);
}

#[test]
fn rollbacks_for_indexes_in_same_family() {
    use crate::ProofListIndex;

    fn indexes(fork: &Fork) -> (ProofListIndex<&Fork, i64>, ProofListIndex<&Fork, i64>) {
        let list1 = fork.get_proof_list(("foo", &1));
        let list2 = fork.get_proof_list(("foo", &2));
        (list1, list2)
    }

    let db = TemporaryDB::new();
    let mut fork = db.fork();
    {
        let (mut list1, mut list2) = indexes(&fork);
        list1.extend(-3..4);
        list2.extend(vec![2, 3, 5, 8]);

        assert_eq!(
            list1.iter().collect::<Vec<_>>(),
            vec![-3, -2, -1, 0, 1, 2, 3]
        );
        assert_eq!(list2.iter().collect::<Vec<_>>(), vec![2, 3, 5, 8]);
    }
    fork.flush();

    {
        let (mut list1, list2) = indexes(&fork);
        assert_eq!(
            list1.iter().collect::<Vec<_>>(),
            vec![-3, -2, -1, 0, 1, 2, 3]
        );
        assert_eq!(list2.iter().collect::<Vec<_>>(), vec![2, 3, 5, 8]);

        list1.set(0, 3);
        list1.set(1, 2);
        assert_eq!(list1.iter().collect::<Vec<_>>(), vec![3, 2, -1, 0, 1, 2, 3]);
        assert_eq!(list2.iter_from(1).collect::<Vec<_>>(), vec![3, 5, 8]);
    }
    fork.rollback();

    let (list1, list2) = indexes(&fork);

    assert_eq!(
        list1.iter().collect::<Vec<_>>(),
        vec![-3, -2, -1, 0, 1, 2, 3]
    );
    assert_eq!(list2.iter().collect::<Vec<_>>(), vec![2, 3, 5, 8]);
}

#[test]
fn clear_view() {
    test_clear_view(&TemporaryDB::new(), IDX_NAME);
}

#[test]
fn clear_prefixed_view() {
    test_clear_view(&TemporaryDB::new(), PREFIXED_IDX);
}

#[test]
fn clear_sibling_views() {
    const IDX_1: (&str, u64) = ("foo", 23);
    const IDX_2: (&str, u64) = ("foo", 42);

    fn assert_view_states<I: RawAccess + Copy>(db_view: I) {
        let view1 = View::new(db_view, IDX_1);
        let view2 = View::new(db_view, IDX_2);

        assert_eq!(view1.get_bytes(&[1]), None);
        assert_eq!(view1.get_bytes(&[0]), Some(vec![5]));
        assert_eq!(view2.get_bytes(&[0]), Some(vec![3]));
        assert_iter(&view1, 1, &[(3, 6)]);
        assert_iter(&view2, 1, &[(2, 4)]);
    }

    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut view1 = View::new(&fork, IDX_1);
        let mut view2 = View::new(&fork, IDX_2);

        view1.put(&vec![0], vec![1]);
        view1.put(&vec![1], vec![2]);
        view2.put(&vec![0], vec![3]);
        view2.put(&vec![2], vec![4]);

        view1.clear();
        assert_eq!(view1.get_bytes(&[0]), None);
        assert_eq!(view2.get_bytes(&[0]), Some(vec![3]));
        assert_iter(&view2, 0, &[(0, 3), (2, 4)]);

        view1.put(&vec![0], vec![5]);
        view1.put(&vec![3], vec![6]);
    }
    db.merge(fork.into_patch()).unwrap();

    assert_view_states(&db.snapshot());

    let fork = db.fork();
    assert_view_states(&fork);
    let mut view1 = View::new(&fork, IDX_1);
    view1.remove(&vec![1]);
    view1.remove(&vec![2]);
    view1.remove(&vec![3]);
    view1.put(&vec![2], vec![7]);
    view1.put(&vec![1], vec![8]);
    assert_iter(&view1, 0, &[(0, 5), (1, 8), (2, 7)]);

    let mut view2 = View::new(&fork, IDX_2);
    view2.clear();
    assert_iter(&view1, 0, &[(0, 5), (1, 8), (2, 7)]);
}

#[test]
#[should_panic]
fn two_mutable_borrows() {
    test_two_mutable_borrows(&TemporaryDB::new(), IDX_NAME);
}

#[test]
#[should_panic]
fn two_mutable_prefixed_borrows() {
    test_two_mutable_borrows(&TemporaryDB::new(), PREFIXED_IDX);
}

#[test]
#[should_panic]
fn mutable_and_immutable_borrows() {
    test_mutable_and_immutable_borrows(&TemporaryDB::new(), IDX_NAME);
}

#[test]
#[should_panic]
fn mutable_and_immutable_prefixed_borrows() {
    test_mutable_and_immutable_borrows(&TemporaryDB::new(), PREFIXED_IDX);
}

#[test]
fn multiple_immutable_borrows_from_fork() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let view1 = View::new(fork.readonly(), IDX_NAME);
    let view2 = View::new(fork.readonly(), IDX_NAME);
    assert_eq!(view1.get_bytes(&[0]), None);
    assert_eq!(view2.get_bytes(&[0]), None);

    let view1 = View::new(fork.readonly(), PREFIXED_IDX);
    let view2 = View::new(fork.readonly(), PREFIXED_IDX);
    assert_eq!(view1.get_bytes(&[0]), None);
    assert_eq!(view2.get_bytes(&[0]), None);
}

#[test]
fn immutable_view_from_fork_reflects_changes_in_fork() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut view = View::new(&fork, IDX_NAME);
        view.put(&vec![1], vec![1, 2, 3, 4]);
        view.put(&vec![2], vec![5, 6, 7]);
    }
    {
        let view = View::new(fork.readonly(), IDX_NAME);
        assert_eq!(view.get_bytes(&[1]), Some(vec![1, 2, 3, 4]));
        let other_view = View::new(fork.readonly(), IDX_NAME);
        assert_eq!(other_view.get_bytes(&[2]), Some(vec![5, 6, 7]));
    }
    View::new(&fork, IDX_NAME).clear();
    let view = View::new(fork.readonly(), IDX_NAME);
    assert_eq!(view.get_bytes(&[1]), None);
}

#[test]
fn immutable_view_from_fork_reads_from_snapshot() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut view = View::new(&fork, IDX_NAME);
        view.put(&vec![1], vec![1, 2, 3, 4]);
        view.put(&vec![2], vec![5, 6, 7]);
    }
    db.merge_sync(fork.into_patch()).unwrap();

    let mut fork = db.fork();
    View::new(&fork, IDX_NAME).put(&vec![1], vec![100]);
    fork.flush();
    View::new(&fork, IDX_NAME).put(&vec![3], vec![200]);

    let fork = fork.readonly();
    // Read from unflushed fork.
    let view = View::new(fork, IDX_NAME);
    assert_eq!(view.get_bytes(&[3]), Some(vec![200]));
    // Read from flushed fork.
    let view = View::new(fork, IDX_NAME);
    assert_eq!(view.get_bytes(&[1]), Some(vec![100]));
    // Read from snapshot.
    let other_view = View::new(fork, IDX_NAME);
    assert_eq!(other_view.get_bytes(&[2]), Some(vec![5, 6, 7]));
}

#[test]
fn mutable_and_immutable_borrows_for_different_views() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let readonly = fork.readonly();

    let immutable_view1 = View::new(readonly, "first");
    let immutable_view2 = View::new(readonly, "second");
    View::new(&fork, "third").put(&vec![1], vec![1, 2, 3]);
    assert_eq!(immutable_view1.get_bytes(&[1]), None);
    let immutable_view3 = View::new(readonly, "third");
    assert_eq!(immutable_view3.get_bytes(&[1]), Some(vec![1, 2, 3]));

    drop(immutable_view1);
    View::new(&fork, "first").put(&vec![1], vec![4, 5, 6]);
    let immutable_view1 = View::new(readonly, "first");
    assert_eq!(immutable_view1.get_bytes(&[1]), Some(vec![4, 5, 6]));
    assert_eq!(immutable_view2.get_bytes(&[1]), None);
}

#[test]
fn views_based_on_rc_fork() {
    fn test_lifetime<T: 'static>(_: T) {}

    const IDX_1: (&str, u64) = ("foo", 23);
    const IDX_2: (&str, u64) = ("foo", 42);

    let db = TemporaryDB::new();
    let fork = Rc::new(db.fork());

    let view1 = View::new(fork.clone(), IDX_1);
    let view2 = View::new(fork.clone(), IDX_2);
    // Test that views have 'static lifetime.
    test_lifetime(view1);
    test_lifetime(view2);

    let mut view1 = View::new(fork.clone(), IDX_1);
    let mut view2 = View::new(fork.clone(), IDX_2);
    view1.put(&vec![0], vec![1]);
    view1.put(&vec![1], vec![2]);
    assert_eq!(view1.get_bytes(&[0]), Some(vec![1]));
    assert_eq!(view1.get_bytes(&[1]), Some(vec![2]));
    view2.put(&vec![0], vec![3]);
    view1.put(&vec![0], vec![3]);
    drop(view1);
    view2.put(&vec![2], vec![4]);
    drop(view2);

    {
        // Check that changes introduced by the both views are reflected in the fork.
        let mut view1 = View::new(&*fork, IDX_1);
        assert_eq!(view1.get_bytes(&[0]), Some(vec![3]));
        view1.remove(&vec![0]);
        let view2 = View::new(fork.clone(), IDX_2);
        assert_eq!(view2.get_bytes(&[2]), Some(vec![4]));
    }

    // ...and that these changes propagate to patch.
    let patch = Rc::try_unwrap(fork).unwrap().into_patch();
    db.merge_sync(patch).unwrap();
    let snapshot = db.snapshot();
    let view1 = View::new(&snapshot, IDX_1);
    assert_eq!(view1.get_bytes(&[0]), None);
    assert_eq!(view1.get_bytes(&[1]), Some(vec![2]));

    let view2 = View::new(&snapshot, IDX_2);
    assert_eq!(view2.get_bytes(&[0]), Some(vec![3]));
    assert_eq!(view2.get_bytes(&[1]), None);
    assert_eq!(view2.get_bytes(&[2]), Some(vec![4]));
}

fn test_metadata(addr: impl Into<IndexAddress>) {
    let addr = addr.into();
    let db = TemporaryDB::new();
    // Creates the index metadata.
    let fork = db.fork();
    ViewWithMetadata::get_or_create(&fork, &addr, IndexType::ProofMap)
        .map_err(drop)
        .unwrap();
    assert!(
        ViewWithMetadata::get_or_create(&db.snapshot(), &addr, IndexType::ProofMap)
            .unwrap()
            .is_phantom()
    );
    db.merge(fork.into_patch()).unwrap();

    let snapshot = db.snapshot();
    let view = ViewWithMetadata::get_or_create(&snapshot, &addr, IndexType::ProofMap).unwrap();
    assert_eq!(view.index_type(), IndexType::ProofMap);
    assert!(!view.is_phantom());

    let fork = db.fork();
    ViewWithMetadata::get_or_create(&fork, &addr, IndexType::ProofMap)
        .map_err(drop)
        .unwrap();
}

#[test]
fn test_metadata_simple() {
    test_metadata("simple");
}

#[test]
fn test_metadata_index_family() {
    test_metadata(("family", "family_id"));
}

fn get_address<T: RawAccess>(view: &View<T>) -> &ResolvedAddress {
    match view {
        View::Real(inner) => &inner.address,
        View::Phantom => panic!("Getting address for a phantom view"),
    }
}

#[test]
fn test_metadata_index_identifiers() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    // Creates the first index metadata.
    let view: View<_> =
        ViewWithMetadata::get_or_create(&fork, &("simple", "family_id").into(), IndexType::Map)
            .map_err(drop)
            .unwrap()
            .into();
    let address = get_address(&view);
    assert_eq!(address.name, "simple");
    let id = address.id.unwrap().get();
    drop(view); // Prevent "multiple mutable borrows" error later

    // Creates the second index metadata.
    let view: View<_> =
        ViewWithMetadata::get_or_create(&fork, &("second", "family_id").into(), IndexType::Map)
            .map_err(drop)
            .unwrap()
            .into();
    let second_address = get_address(&view);
    assert_eq!(second_address.name, "second");
    assert_eq!(second_address.id.unwrap().get(), id + 1);

    // Recreates the first index instance.
    let view: View<_> =
        ViewWithMetadata::get_or_create(&fork, &("simple", "family_id").into(), IndexType::Map)
            .map_err(drop)
            .unwrap()
            .into();
    let recreated_address = get_address(&view);
    assert_eq!(recreated_address.name, "simple");
    assert_eq!(recreated_address.id.unwrap().get(), id);
}

#[test]
fn test_metadata_in_migrated_indexes() {
    let db = TemporaryDB::new();
    let fork = db.fork();

    let view: View<_> = ViewWithMetadata::get_or_create(&fork, &"simple".into(), IndexType::Map)
        .map_err(drop)
        .unwrap()
        .into();
    let address = get_address(&view);
    assert_eq!(address.name, "simple");
    let old_id = address.id.unwrap().get();

    let mut addr = IndexAddress::from_root("simple");
    addr.set_in_migration();
    let view: View<_> = ViewWithMetadata::get_or_create(&fork, &addr, IndexType::List)
        .map_err(drop)
        .unwrap()
        .into();
    let migrated_address = get_address(&view);
    assert_eq!(migrated_address.name, "simple");
    let new_id = migrated_address.id.unwrap().get();
    assert_ne!(old_id, new_id);
}

#[test]
fn test_metadata_incorrect_index_type() {
    let db = TemporaryDB::new();
    let fork = db.fork();

    ViewWithMetadata::get_or_create(&fork, &"simple".into(), IndexType::Map)
        .map_err(drop)
        .unwrap();
    ViewWithMetadata::get_or_create(&fork, &"simple".into(), IndexType::List)
        .map(drop)
        .unwrap_err();
}

#[test]
fn test_metadata_index_wrong_type() {
    use crate::{
        access::{AccessError, AccessErrorKind, FromAccess},
        ListIndex,
    };

    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut map = fork.get_map("simple");
        map.put(&1, vec![1, 2, 3]);
    }

    db.merge(fork.into_patch()).unwrap();
    // Attempt to create an index with the wrong type (`List` instead of `Map`).
    let snapshot = db.snapshot();
    let err = ListIndex::<_, Vec<u8>>::from_access(&snapshot, "simple".into()).unwrap_err();

    assert_matches!(
        err,
        AccessError { ref addr, kind: AccessErrorKind::WrongIndexType { .. } }
            if *addr == IndexAddress::from("simple")
    );
}

#[test]
fn test_valid_tombstone() {
    use crate::{
        access::{AccessErrorKind, FromAccess},
        migration::Migration,
        ListIndex,
    };

    let db = TemporaryDB::new();
    let fork = db.fork();
    let migration = Migration::new("foo", &fork);

    // Valid tombstone in a fork.
    migration.create_tombstone("bar");
    // Check that index cannot be reinterpreted with another type.
    let err = ListIndex::<_, u64>::from_access(migration, "bar".into()).unwrap_err();
    assert_matches!(
        err.kind,
        AccessErrorKind::WrongIndexType { actual, .. } if actual == IndexType::Tombstone
    );

    // ...even after the fork is merged.
    db.merge(fork.into_patch()).unwrap();
    let snapshot = db.snapshot();
    let migration = Migration::new("foo", &snapshot);
    let err = ListIndex::<_, u64>::from_access(migration, "bar".into()).unwrap_err();
    assert_matches!(
        err.kind,
        AccessErrorKind::WrongIndexType { actual, .. } if actual == IndexType::Tombstone
    );
}

#[test]
fn test_invalid_tombstone() {
    use crate::access::{Access, AccessErrorKind};

    let db = TemporaryDB::new();
    let fork = db.fork();
    // A tombstone cannot be created outside the migration!
    let err = fork
        .get_or_create_view("foo.bar".into(), IndexType::Tombstone)
        .unwrap_err();
    assert_matches!(err.kind, AccessErrorKind::InvalidTombstone);
}

#[test]
#[ignore] // TODO: fix test [ECR-2869]
fn multiple_patch() {
    fn list_index(view: &Fork) -> ListIndex<&Fork, u64> {
        view.get_list("list_index")
    }

    let db = TemporaryDB::new();
    // create first patch
    let fork = db.fork();
    {
        let mut index = list_index(&fork);
        index.push(1);
        index.push(3);
        index.push(4);
    }
    let patch1 = fork.into_patch();
    // create second patch
    let fork = db.fork();
    {
        let mut index = list_index(&fork);
        index.push(10);
    }
    let patch2 = fork.into_patch();

    db.merge(patch1).unwrap();
    db.merge(patch2).unwrap();
    let snapshot = db.snapshot();
    let index: ListIndex<_, u64> = snapshot.get_list("list_index");
    assert_eq!(index.len() as usize, index.iter().count());
}

#[test]
fn valid_index_name() {
    assert!(check_valid_name("index_name"));
    assert!(check_valid_name("_index_name"));
    assert!(check_valid_name("AinDex_name_"));
    assert!(check_valid_name("core.index_name1Z"));
    assert!(check_valid_name("configuration.indeX_1namE"));
    assert!(check_valid_name("1index_Name"));
    assert!(check_valid_name("index-name"));
    assert!(check_valid_name("_index-name"));
    assert!(check_valid_name("indeX_1namE-"));

    assert!(!check_valid_name(
        "\u{438}\u{43d}\u{434}\u{435}\u{43a}\u{441}_name_"
    ));
    assert!(!check_valid_name("core.index_\u{438}\u{43c}\u{44f}3"));
    assert!(!check_valid_name("1in!dex_Name"));
    assert!(!check_valid_name("space name"));
    assert!(!check_valid_name(" space "));
}

#[test]
fn valid_name_for_url() {
    assert_valid_name_url("service_name");
    assert_valid_name_url("service_name\\");
    assert_valid_name_url("service name");
    assert_valid_name_url("/service_name");
    assert_valid_name_url("1Service_name");
    assert_valid_name_url("core.service_name");
}

#[test]
#[should_panic(expected = "Invalid characters used in name")]
fn invalid_name_panic() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let _: ListIndex<_, u8> = fork.get_list("ind\u{435}x-name");
}

fn assert_valid_name_url(name: &str) {
    let urlencoded: String = byte_serialize(name.as_bytes()).collect();
    assert_eq!(is_valid_identifier(name), name == urlencoded)
}

fn check_valid_name(name: &str) -> bool {
    let db = TemporaryDB::new();
    let catch_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let fork = db.fork();
        let _: ListIndex<_, u8> = fork.get_list(name.as_ref());
    }));
    catch_result.is_ok()
}

#[test]
fn fork_from_patch() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut index = fork.get_list("index");
        index.push(1);
        index.push(2);
        index.push(3);
        let last = index.pop();
        assert_eq!(last, Some(3));
        index.set(1, 5);
    }

    let patch = fork.into_patch();
    let fork: Fork = patch.into();
    {
        let index = fork.get_list("index");
        assert_eq!(index.get(0), Some(1));
        assert_eq!(index.get(1), Some(5));
        assert_eq!(index.get(2), None);

        let items: Vec<i32> = index.iter().collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items, vec![1, 5]);
    }

    db.merge(fork.into_patch())
        .expect("Fork created from patch should be merged successfully");
}
