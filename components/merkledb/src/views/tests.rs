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
use url::form_urlencoded::byte_serialize;

use std::panic;

use crate::{
    db,
    views::{is_valid_name, IndexAccess, IndexAddress, IndexBuilder, IndexType, View},
    Database, DbOptions, Fork, ListIndex, RocksDB, TemporaryDB,
};

const IDX_NAME: &str = "idx_name";
const PREFIXED_IDX: (&str, &[u8]) = ("idx", &[1_u8, 2, 3] as &[u8]);

fn assert_iter<T: IndexAccess>(view: &View<T>, from: u8, assumed: &[(u8, u8)]) {
    let mut iter = view.iter_bytes(&[from]);
    let mut values = Vec::new();
    while let Some((k, v)) = iter.next() {
        values.push((k[0], v[0]));
    }

    assert_eq!(values, assumed);
}

fn assert_initial_state<T: IndexAccess>(view: &View<T>) {
    assert_eq!(view.get_bytes(&[1]), Some(vec![1]));
    assert_eq!(view.get_bytes(&[2]), Some(vec![2]));
    assert_eq!(view.get_bytes(&[3]), Some(vec![3]));
    assert_eq!(view.get_bytes(&[4]), None);
}

fn _changelog<T: Database, I: Into<IndexAddress> + Copy>(db: &T, address: I) {
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
    const IDX_1: (&str, &[u8]) = ("foo", &[1_u8, 2] as &[u8]);
    const IDX_2: (&str, &[u8]) = ("foo", &[1_u8, 3] as &[u8]);

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

fn _two_mutable_borrows<T, I>(db: &T, address: I)
where
    T: Database,
    I: Into<IndexAddress> + Copy,
{
    let fork = db.fork();

    let view1 = View::new(&fork, address);
    let view2 = View::new(&fork, address);
    assert_eq!(view1.get_bytes(&[0]), None);
    assert_eq!(view2.get_bytes(&[0]), None);
}

fn _mutable_and_immutable_borrows<T, I>(db: &T, address: I)
where
    T: Database,
    I: Into<IndexAddress> + Copy,
{
    let fork = db.fork();

    let view1 = View::new(&fork, address);
    let view2 = View::new(&fork, address);
    assert_eq!(view1.get_bytes(&[0]), None);
    assert_eq!(view2.get_bytes(&[0]), None);
}

fn _clear_view<T, I>(db: &T, address: I)
where
    T: Database,
    I: Into<IndexAddress> + Copy,
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

fn _fork_iter<T, I>(db: &T, address: I)
where
    T: Database,
    I: Into<IndexAddress> + Copy,
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

    let view = View::new(&snapshot, IndexAddress::with_root(db::DB_METADATA));
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
            let mut view = View::new(&fork, IndexAddress::with_root(db::DB_METADATA));
            view.put(db::VERSION_NAME, 2_u8);
        }
        db.merge(fork.into_patch()).unwrap();
    }
    // Tries to open modified database.
    RocksDB::open(&dir, &opts).unwrap();
}

#[test]
fn fork_iter() {
    _fork_iter(&TemporaryDB::new(), IDX_NAME);
}

#[test]
fn fork_iter_prefixed() {
    _fork_iter(&TemporaryDB::new(), PREFIXED_IDX);
}

#[test]
fn changelog() {
    _changelog(&TemporaryDB::new(), IDX_NAME);
}

#[test]
fn changelog_prefixed() {
    _changelog(&TemporaryDB::new(), PREFIXED_IDX);
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

#[cfg(test)]
#[test]
fn multiple_indexes() {
    use crate::{ListIndex, MapIndex};

    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut list: ListIndex<_, u32> = ListIndex::new(IDX_NAME, &fork);
        let mut map = MapIndex::new_in_family("idx", &3, &fork);

        for i in 0..10 {
            list.push(i);
            if i % 2 == 0 {
                map.put(&i, "??".to_owned());
            }
        }
    }
    db.merge(fork.into_patch()).unwrap();

    {
        let snapshot = db.snapshot();
        let list: ListIndex<_, u32> = ListIndex::new(IDX_NAME, &snapshot);
        let map: MapIndex<_, u32, String> = MapIndex::new_in_family("idx", &3, &snapshot);

        assert_eq!(list.len(), 10);
        assert!(map.values().all(|val| val == "??"));
    }

    let fork = db.fork();
    let list: ListIndex<_, u32> = ListIndex::new(IDX_NAME, &fork);
    let mut map = MapIndex::new_in_family("idx", &3, &fork);
    for item in &list {
        map.put(&item, item.to_string());
    }

    assert_eq!(map.values().count(), 10);
    assert!(map.iter_from(&3).all(|(k, v)| k < 10 && v == k.to_string()));
}

#[test]
fn views_in_same_family() {
    const IDX_1: (&str, &[u8]) = ("foo", &[1_u8, 2] as &[u8]);
    const IDX_2: (&str, &[u8]) = ("foo", &[1_u8, 3] as &[u8]);

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
        let list1 = ProofListIndex::new_in_family("foo", &1, fork);
        let list2 = ProofListIndex::new_in_family("foo", &2, fork);

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
    _clear_view(&TemporaryDB::new(), IDX_NAME);
}

#[test]
fn clear_prefixed_view() {
    _clear_view(&TemporaryDB::new(), PREFIXED_IDX);
}

#[test]
fn clear_sibling_views() {
    const IDX_1: (&str, &[u8]) = ("foo", &[1_u8, 2] as &[u8]);
    const IDX_2: (&str, &[u8]) = ("foo", &[1_u8, 3] as &[u8]);

    fn assert_view_states<I: IndexAccess + Copy>(db_view: I) {
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
    _two_mutable_borrows(&TemporaryDB::new(), IDX_NAME);
}

#[test]
#[should_panic]
fn two_mutable_prefixed_borrows() {
    _two_mutable_borrows(&TemporaryDB::new(), PREFIXED_IDX);
}

#[test]
#[should_panic]
fn mutable_and_immutable_borrows() {
    _mutable_and_immutable_borrows(&TemporaryDB::new(), IDX_NAME);
}

#[test]
#[should_panic]
fn mutable_and_immutable_prefixed_borrows() {
    _mutable_and_immutable_borrows(&TemporaryDB::new(), PREFIXED_IDX);
}

#[test]
fn test_metadata_index_usual_correct() {
    let db = TemporaryDB::new();
    // Creates the index metadata.
    IndexBuilder::new(&db.fork())
        .index_name("simple")
        .index_type(IndexType::ProofMap)
        .build::<()>();
    // Checks the index metadata.
    IndexBuilder::new(&db.snapshot())
        .index_name("simple")
        .index_type(IndexType::ProofMap)
        .build::<()>();
}

#[test]
fn test_metadata_index_family_correct() {
    let db = TemporaryDB::new();
    // Creates the index metadata.
    let fork = db.fork();
    IndexBuilder::new(&fork)
        .index_name("simple")
        .family_id("family")
        .index_type(IndexType::ProofMap)
        .build::<()>();
    db.merge(fork.into_patch()).unwrap();
    // Checks the index metadata.
    IndexBuilder::new(&db.snapshot())
        .index_name("simple")
        .family_id("family")
        .index_type(IndexType::ProofMap)
        .build::<()>();
}

#[test]
fn test_metadata_index_identifiers() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    // Creates the first index metadata.
    {
        let (view, _state) = IndexBuilder::new(&fork)
            .index_name("simple")
            .family_id("family")
            .index_type(IndexType::ProofMap)
            .build::<()>();
        assert_eq!(view.address, IndexAddress::new().append_bytes(&0_u64));
    }

    // Creates the second index metadata.
    {
        let (view, _state) = IndexBuilder::new(&fork)
            .index_name("second")
            .family_id("family")
            .index_type(IndexType::ProofMap)
            .build::<()>();
        assert_eq!(view.address, IndexAddress::new().append_bytes(&1_u64));
    }

    // Tries to create the first index instance.
    {
        let (view, _state) = IndexBuilder::new(&fork)
            .index_name("simple")
            .family_id("family")
            .index_type(IndexType::ProofMap)
            .build::<()>();
        assert_eq!(view.address, IndexAddress::new().append_bytes(&0_u64));
    }
}

#[test]
fn test_index_builder_without_type() {
    let db = TemporaryDB::new();
    // Creates the index metadata.
    let fork = db.fork();
    IndexBuilder::new(&fork).index_name("simple").build::<()>();
    db.merge(fork.into_patch()).unwrap();
    // Checks the index metadata.
    IndexBuilder::new(&db.snapshot())
        .index_name("simple")
        .index_type(IndexType::Unknown)
        .build::<()>();
}

#[test]
#[should_panic(expected = "Index name must not be empty")]
fn test_index_builder_without_name() {
    let db = TemporaryDB::new();
    // Creates the index metadata.
    let fork = db.fork();
    IndexBuilder::new(&fork).build::<()>();
}

#[test]
#[should_panic(expected = "Index type doesn't match specified")]
fn test_metadata_index_usual_incorrect() {
    let db = TemporaryDB::new();
    // Creates the index metadata.
    let fork = db.fork();
    IndexBuilder::new(&fork)
        .index_type(IndexType::ProofMap)
        .index_name("simple")
        .build::<()>();
    db.merge(fork.into_patch()).unwrap();
    // Checks the index metadata.
    IndexBuilder::new(&db.snapshot())
        .index_type(IndexType::ProofList)
        .index_name("simple")
        .build::<()>();
}

#[test]
#[should_panic(expected = "Index type doesn't match specified")]
fn test_metadata_index_family_incorrect() {
    let db = TemporaryDB::new();
    // Creates the index metadata.
    let fork = db.fork();
    IndexBuilder::new(&fork)
        .index_type(IndexType::ProofMap)
        .index_name("simple")
        .family_id("family")
        .build::<()>();
    db.merge(fork.into_patch()).unwrap();
    // Checks the index metadata.
    IndexBuilder::new(&db.snapshot())
        .index_type(IndexType::Map)
        .index_name("simple")
        .family_id("family")
        .build::<()>();
}

#[test]
#[ignore]
//TODO: fix test [ECR-2869]
fn multiple_patch() {
    use crate::ListIndex;

    fn list_index<View: IndexAccess>(view: View) -> ListIndex<View, u64> {
        ListIndex::new("list_index", view)
    }

    let db = TemporaryDB::new();
    // create first patch
    let patch1 = {
        let fork = db.fork();
        {
            let mut index = list_index(&fork);
            index.push(1);
            index.push(3);
            index.push(4);
        }
        fork.into_patch()
    };
    // create second patch
    let patch2 = {
        let fork = db.fork();
        {
            let mut index = list_index(&fork);
            index.push(10);
        }
        fork.into_patch()
    };

    db.merge(patch1).unwrap();
    db.merge(patch2).unwrap();
    let snapshot = db.snapshot();
    let index = list_index(&snapshot);
    let iter = index.iter();
    assert_eq!(index.len() as usize, iter.count());
}

#[test]
fn valid_index_name() {
    assert!(check_valid_name("index_name"));
    assert!(check_valid_name("_index_name"));
    assert!(check_valid_name("AinDex_name_"));
    assert!(check_valid_name("core.index_name1Z"));
    assert!(check_valid_name("configuration.indeX_1namE"));
    assert!(check_valid_name("1index_Namez"));
    assert!(check_valid_name("index-name"));
    assert!(check_valid_name("_index-name"));
    assert!(check_valid_name("indeX_1namE-"));

    assert!(!check_valid_name(
        "\u{438}\u{43d}\u{434}\u{435}\u{43a}\u{441}_name_"
    ));
    assert!(!check_valid_name("core.index_\u{438}\u{43c}\u{44f}3"));
    assert!(!check_valid_name("1in!dex_Namez"));
    assert!(!check_valid_name("space name"));
    assert!(!check_valid_name(" space "));
}

#[test]
fn valid_name_for_url() {
    assert_valid_name_url("service_name");
    assert_valid_name_url("service_name\\");
    assert_valid_name_url("sarvice_name");
    assert_valid_name_url("/service_name");
    assert_valid_name_url("1Service_name");
    assert_valid_name_url("core.service_name");
}

#[test]
#[should_panic(expected = "Wrong characters using in name. Use: a-zA-Z0-9 and _")]
fn invalid_name_panic() {
    let db = TemporaryDB::new();
    let snapshot = db.snapshot();
    let _: ListIndex<_, u8> = ListIndex::new("ind\u{435}x-name", &snapshot);
}

fn assert_valid_name_url(name: &str) {
    let urlencoded: String = byte_serialize(name.as_bytes()).collect();
    assert_eq!(is_valid_name(name), name == urlencoded)
}

fn check_valid_name<S: AsRef<str>>(name: S) -> bool {
    check_index_name(name).is_ok()
}

fn check_index_name<S: AsRef<str>>(name: S) -> Result<(), ()> {
    let db = TemporaryDB::new();
    let snapshot = db.snapshot();

    let catch_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let _: ListIndex<_, u8> = ListIndex::new(name.as_ref(), &snapshot);
    }));

    catch_result.map_err(|_| ())
}
