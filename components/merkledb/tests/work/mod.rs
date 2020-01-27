//! Operations on indexes shared between `backup` and `migration` tests.

use proptest::{
    collection::vec, option, prop_assert_eq, prop_oneof, sample, strategy, strategy::Strategy,
    test_runner::TestCaseResult,
};

use std::{collections::BTreeMap, iter::FromIterator};

use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    IndexAddress, IndexType,
};

/// Possible index names.
pub const INDEX_NAMES: &[&str] = &[
    "foo",
    "bar",
    "b",
    "overly_long_prefix_still_should_work_though",
];

pub fn work_on_index<T>(
    fork: T,
    addr: IndexAddress,
    mut index_type: IndexType,
    value: Option<Vec<u8>>,
) -> IndexType
where
    T: Access,
    T::Base: RawAccessMut,
{
    if let Some(real_type) = fork.index_type(addr.clone()) {
        index_type = real_type;
    }

    match index_type {
        IndexType::Entry => {
            let mut entry = fork.get_entry(addr);
            if let Some(val) = value {
                entry.set(val);
            } else {
                entry.remove();
            }
        }
        IndexType::ProofEntry => {
            let mut entry = fork.get_proof_entry(addr);
            if let Some(val) = value {
                entry.set(val);
            } else {
                entry.remove();
            }
        }

        IndexType::List => {
            let mut list = fork.get_list(addr);
            if let Some(val) = value {
                list.push(val);
            } else {
                list.clear();
            }
        }
        IndexType::ProofList => {
            let mut list = fork.get_proof_list(addr);
            if let Some(val) = value {
                list.push(val);
            } else {
                list.clear();
            }
        }

        IndexType::Map => {
            let mut map = fork.get_map(addr);
            if let Some(val) = value {
                let key = val[0];
                map.put(&key, val);
            } else {
                map.clear();
            }
        }
        IndexType::ProofMap => {
            let mut map = fork.get_proof_map(addr);
            if let Some(val) = value {
                let key = val[0];
                map.put(&key, val);
            } else {
                map.clear();
            }
        }

        _ => {}
    }

    index_type
}

/// Generates an `IndexAddress` optionally placed in a group.
pub fn generate_address() -> impl Strategy<Value = IndexAddress> {
    let index_name = sample::select(INDEX_NAMES).prop_map(IndexAddress::from_root);
    prop_oneof![
        // Non-prefixed addresses
        index_name.clone(),
        // Prefixed addresses
        (index_name, 1_u8..8).prop_map(|(addr, prefix)| addr.append_key(&prefix)),
    ]
}

pub fn generate_index_type() -> impl Strategy<Value = IndexType> {
    prop_oneof![
        strategy::Just(IndexType::Entry),
        strategy::Just(IndexType::ProofEntry),
        strategy::Just(IndexType::List),
        strategy::Just(IndexType::ProofList),
        strategy::Just(IndexType::Map),
        strategy::Just(IndexType::ProofMap),
    ]
}

/// Generates a value to place in the index. if `None` is generated, the index will be cleared
/// instead.
pub fn generate_value() -> impl Strategy<Value = Option<Vec<u8>>> {
    option::weighted(0.8, vec(0_u8..4, 1..=1))
}

#[derive(Debug, Clone)]
pub struct IndexData {
    pub ty: IndexType,
    pub values: Vec<Vec<u8>>,
}

impl IndexData {
    pub fn check<S>(&self, snapshot: S, addr: IndexAddress) -> TestCaseResult
    where
        S: Access,
    {
        match self.ty {
            IndexType::Entry => {
                let val = snapshot.get_entry::<_, Vec<u8>>(addr).get();
                prop_assert_eq!(val.as_ref(), self.values.last());
            }
            IndexType::ProofEntry => {
                let val = snapshot.get_proof_entry::<_, Vec<u8>>(addr).get();
                prop_assert_eq!(val.as_ref(), self.values.last());
            }

            IndexType::List => {
                let list = snapshot.get_list::<_, Vec<u8>>(addr);
                prop_assert_eq!(list.len(), self.values.len() as u64);
                let values = list.iter().collect::<Vec<_>>();
                prop_assert_eq!(&values, &self.values);
            }
            IndexType::ProofList => {
                let list = snapshot.get_proof_list::<_, Vec<u8>>(addr);
                prop_assert_eq!(list.len(), self.values.len() as u64);
                let values = list.iter().collect::<Vec<_>>();
                prop_assert_eq!(&values, &self.values);
            }

            IndexType::Map => {
                let map = snapshot.get_map::<_, u8, Vec<u8>>(addr);
                let expected_map =
                    BTreeMap::from_iter(self.values.iter().map(|val| (val[0], val.clone())));
                // Using `Vec<_>` allows to test for duplicate entries during iteration etc.
                let expected_map: Vec<_> = expected_map.into_iter().collect();
                prop_assert_eq!(map.iter().collect::<Vec<_>>(), expected_map);
            }
            IndexType::ProofMap => {
                let map = snapshot.get_proof_map::<_, u8, Vec<u8>>(addr);
                let expected_map =
                    BTreeMap::from_iter(self.values.iter().map(|val| (val[0], val.clone())));
                let expected_map: Vec<_> = expected_map.into_iter().collect();
                prop_assert_eq!(map.iter().collect::<Vec<_>>(), expected_map);
            }

            _ => {}
        }
        Ok(())
    }
}
