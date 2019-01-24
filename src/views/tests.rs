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

use crate::{
    views::{IndexAccess, IndexAddress, IndexBuilder, View},
    Database, Fork,
};

const IDX_NAME: &str = "idx_name";
const PREFIXED_IDX: (&str, &[u8]) = ("idx", &[1u8, 2, 3] as &[u8]);

fn create_view<T, I>(db_view: T, address: I) -> View<T>
where
    T: IndexAccess,
    I: Into<IndexAddress>,
{
    IndexBuilder::from_address(db_view, address.into()).build()
}

fn assert_iter<T: IndexAccess>(view: &View<T>, from: u8, assumed: &[(u8, u8)]) {
    let mut iter = view.iter_bytes(&[from]);
    let mut values = Vec::new();
    while let Some((k, v)) = iter.next() {
        values.push((k[0], v[0]));
    }

    assert_eq!(values, assumed);
}

fn fork_iter<T: Database, I: Into<IndexAddress> + Copy>(db: T, address: I) {
    let fork = db.fork();
    {
        let view = create_view(&fork, address);
        let mut view = view;
        view.put(&vec![10], vec![10]);
        view.put(&vec![20], vec![20]);
        view.put(&vec![30], vec![30]);
        assert!(view.contains_raw_key(&[10]));
    }
    db.merge(fork.into_patch()).unwrap();

    let fork = db.fork();
    let mut view = create_view(&fork, address);
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
    let mut view = create_view(&fork, address);

    view.put(&vec![10], vec![11]);
    assert_iter(&view, 0, &[(10, 11), (20, 20), (30, 30)]);
    view.put(&vec![30], vec![31]);
    assert_iter(&view, 0, &[(10, 11), (20, 20), (30, 31)]);

    // Deleted
    let fork = db.fork();
    let mut view = create_view(&fork, address);

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
    let mut view = create_view(&fork, address);

    view.remove(&vec![5]);
    assert_iter(&view, 0, &[(10, 10), (20, 20), (30, 30)]);
    view.remove(&vec![15]);
    assert_iter(&view, 0, &[(10, 10), (20, 20), (30, 30)]);
    view.remove(&vec![35]);
    assert_iter(&view, 0, &[(10, 10), (20, 20), (30, 30)]);
}

fn changelog<T: Database, I: Into<IndexAddress> + Copy>(db: T, address: I) {
    let mut fork = db.fork();
    {
        let mut view = create_view(&fork, address);
        view.put(&vec![1], vec![1]);
        view.put(&vec![2], vec![2]);
        view.put(&vec![3], vec![3]);

        assert_eq!(view.get_bytes(&[1]), Some(vec![1]));
        assert_eq!(view.get_bytes(&[2]), Some(vec![2]));
        assert_eq!(view.get_bytes(&[3]), Some(vec![3]));
    }
    fork.flush();

    {
        let mut view = create_view(&fork, address);
        assert_eq!(view.get_bytes(&[1]), Some(vec![1]));
        assert_eq!(view.get_bytes(&[2]), Some(vec![2]));
        assert_eq!(view.get_bytes(&[3]), Some(vec![3]));

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
        let view = create_view(&fork, address);
        assert_eq!(view.get_bytes(&[1]), Some(vec![1]));
        assert_eq!(view.get_bytes(&[2]), Some(vec![2]));
        assert_eq!(view.get_bytes(&[3]), Some(vec![3]));
        assert_eq!(view.get_bytes(&[4]), None);
    }
    fork.flush();

    {
        let mut view = create_view(&fork, address);
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
        let view = create_view(&fork, address);
        assert_eq!(view.get_bytes(&[1]), Some(vec![1]));
        assert_eq!(view.get_bytes(&[2]), Some(vec![2]));
        assert_eq!(view.get_bytes(&[3]), Some(vec![3]));
        assert_eq!(view.get_bytes(&[4]), None);
    }

    create_view(&fork, address).put(&vec![2], vec![20]);
    fork.flush();
    create_view(&fork, address).put(&vec![3], vec![30]);
    fork.rollback();

    let view = create_view(&fork, address);
    assert_eq!(view.get_bytes(&[1]), Some(vec![1]));
    assert_eq!(view.get_bytes(&[2]), Some(vec![20]));
    assert_eq!(view.get_bytes(&[3]), Some(vec![3]));
    assert_eq!(view.get_bytes(&[4]), None);
}

fn multiple_views<T: Database>(db: T) {
    let fork = db.fork();

    {
        // Writing to multiple views at the same time
        let mut view = create_view(&fork, IDX_NAME);
        let mut prefixed_view = create_view(&fork, PREFIXED_IDX);

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
        let view = create_view(&snapshot, IDX_NAME);
        let prefixed_view = create_view(&snapshot, PREFIXED_IDX);

        assert_iter(&view, 0, &[(1, 10), (2, 20), (3, 30)]);
        assert_iter(&prefixed_view, 0, &[(1, 30), (3, 40), (5, 50)]);
    }

    let fork = db.fork();
    {
        // Reading from one view and writing to other
        let view = create_view(&fork, IDX_NAME);
        let mut prefixed_view = create_view(&fork, PREFIXED_IDX);

        assert_iter(&view, 0, &[(1, 10), (2, 20), (3, 30)]);
        assert_iter(&prefixed_view, 0, &[(1, 30), (3, 40), (5, 50)]);

        prefixed_view.remove(&vec![3]);
        prefixed_view.put(&vec![2], vec![60]);
        assert_iter(&view, 0, &[(1, 10), (2, 20), (3, 30)]);
        assert_iter(&prefixed_view, 0, &[(1, 30), (2, 60), (5, 50)]);
    }
}

fn multiple_indexes<T: Database>(db: T) {
    use crate::{ListIndex, MapIndex};

    let fork = db.fork();
    {
        let mut list = ListIndex::new(IDX_NAME, &fork);
        let mut map = MapIndex::new_in_family("idx", &3, &fork);

        for i in 0..10 as u32 {
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

fn views_in_same_family<T: Database>(db: T) {
    const IDX_1: (&str, &[u8]) = ("foo", &[1u8, 2] as &[u8]);
    const IDX_2: (&str, &[u8]) = ("foo", &[1u8, 3] as &[u8]);

    let mut fork = db.fork();
    {
        let mut view1 = create_view(&fork, IDX_1);
        let mut view2 = create_view(&fork, IDX_2);

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
        let mut view1 = create_view(&fork, IDX_1);
        let view2 = create_view(&fork, IDX_2);

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
    let view1 = create_view(&snapshot, IDX_1);
    let view2 = create_view(&snapshot, IDX_2);

    assert_iter(&view1, 0, &[(1, 10), (2, 30), (3, 40)]);
    assert_iter(&view2, 0, &[(0, 0), (1, 2), (2, 4)]);
}

fn rollbacks_for_indexes_in_same_family<T: Database>(db: T) {
    use crate::ProofListIndex;

    fn indexes(fork: &Fork) -> (ProofListIndex<&Fork, i64>, ProofListIndex<&Fork, i64>) {
        let list1 = ProofListIndex::new_in_family("foo", &1, fork);
        let list2 = ProofListIndex::new_in_family("foo", &2, fork);

        (list1, list2)
    }

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

fn two_mutable_borrows<T, I>(db: T, address: I)
where
    T: Database,
    I: Into<IndexAddress> + Copy,
{
    let fork = db.fork();

    let view1 = create_view(&fork, address);
    let view2 = create_view(&fork, address);
    assert_eq!(view1.get_bytes(&[0]), None);
    assert_eq!(view2.get_bytes(&[0]), None);
}

fn mutable_and_immutable_borrows<T, I>(db: T, address: I)
where
    T: Database,
    I: Into<IndexAddress> + Copy,
{
    let fork = db.fork();

    let view1 = create_view(&fork, address);
    let view2 = create_view(&fork, address);
    assert_eq!(view1.get_bytes(&[0]), None);
    assert_eq!(view2.get_bytes(&[0]), None);
}

fn clear_view<T, I>(db: T, address: I)
where
    T: Database,
    I: Into<IndexAddress> + Copy,
{
    let fork = db.fork();
    {
        let mut view = create_view(&fork, address);
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
        let view = create_view(&snapshot, address);

        assert_eq!(view.get_bytes(&[1]), Some(vec![5]));
        assert_iter(&view, 0, &[(1, 5), (3, 6)]);
        assert_iter(&view, 2, &[(3, 6)]);
    }

    let fork = db.fork();
    {
        let mut view = create_view(&fork, address);
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
        let view = create_view(&fork, address);

        assert_eq!(view.get_bytes(&[1]), None);
        assert_eq!(view.get_bytes(&[3]), Some(vec![0]));
        assert_iter(&view, 0, &[(3, 0), (4, 0)]);
        assert_iter(&view, 4, &[(4, 0)]);
    }

    db.merge(fork.into_patch()).unwrap();
    let snapshot = db.snapshot();
    let view = create_view(&snapshot, address);
    assert_iter(&view, 0, &[(3, 0), (4, 0)]);
    assert_iter(&view, 4, &[(4, 0)]);
}

fn clear_sibling_views<T: Database>(db: T) {
    const IDX_1: (&str, &[u8]) = ("foo", &[1u8, 2] as &[u8]);
    const IDX_2: (&str, &[u8]) = ("foo", &[1u8, 3] as &[u8]);

    let fork = db.fork();
    {
        let mut view1 = create_view(&fork, IDX_1);
        let mut view2 = create_view(&fork, IDX_2);

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

    fn assert_view_states<I: IndexAccess + Copy>(db_view: I) {
        let view1 = create_view(db_view, IDX_1);
        let view2 = create_view(db_view, IDX_2);

        assert_eq!(view1.get_bytes(&[1]), None);
        assert_eq!(view1.get_bytes(&[0]), Some(vec![5]));
        assert_eq!(view2.get_bytes(&[0]), Some(vec![3]));
        assert_iter(&view1, 1, &[(3, 6)]);
        assert_iter(&view2, 1, &[(2, 4)]);
    }

    assert_view_states(&db.snapshot());

    let fork = db.fork();
    assert_view_states(&fork);
    let mut view1 = create_view(&fork, IDX_1);
    view1.remove(&vec![1]);
    view1.remove(&vec![2]);
    view1.remove(&vec![3]);
    view1.put(&vec![2], vec![7]);
    view1.put(&vec![1], vec![8]);
    assert_iter(&view1, 0, &[(0, 5), (1, 8), (2, 7)]);

    let mut view2 = create_view(&fork, IDX_2);
    view2.clear();
    assert_iter(&view1, 0, &[(0, 5), (1, 8), (2, 7)]);
}

mod temporarydb {
    use super::{IDX_NAME, PREFIXED_IDX};
    use crate::TemporaryDB;

    fn database() -> TemporaryDB {
        TemporaryDB::new()
    }

    #[test]
    fn fork_iter() {
        super::fork_iter(database(), IDX_NAME);
    }

    #[test]
    fn fork_iter_prefixed() {
        super::fork_iter(database(), PREFIXED_IDX);
    }

    #[test]
    fn changelog() {
        super::changelog(database(), IDX_NAME);
    }

    #[test]
    fn changelog_prefixed() {
        super::changelog(database(), PREFIXED_IDX);
    }

    #[test]
    fn multiple_views() {
        super::multiple_views(database());
    }

    #[test]
    fn multiple_indexes() {
        super::multiple_indexes(database());
    }

    #[test]
    fn views_in_same_family() {
        super::views_in_same_family(database());
    }

    #[test]
    fn rollbacks_for_indexes_in_same_family() {
        super::rollbacks_for_indexes_in_same_family(database());
    }

    #[test]
    fn clear_view() {
        super::clear_view(database(), IDX_NAME);
    }

    #[test]
    fn clear_prefixed_view() {
        super::clear_view(database(), PREFIXED_IDX);
    }

    #[test]
    fn clear_sibling_views() {
        super::clear_sibling_views(database());
    }

    #[test]
    #[should_panic]
    fn two_mutable_borrows() {
        super::two_mutable_borrows(database(), IDX_NAME);
    }

    #[test]
    #[should_panic]
    fn two_mutable_prefixed_borrows() {
        super::two_mutable_borrows(database(), PREFIXED_IDX);
    }

    #[test]
    #[should_panic]
    fn mutable_and_immutable_borrows() {
        super::mutable_and_immutable_borrows(database(), IDX_NAME);
    }

    #[test]
    #[should_panic]
    fn mutable_and_immutable_prefixed_borrows() {
        super::mutable_and_immutable_borrows(database(), PREFIXED_IDX);
    }
}
