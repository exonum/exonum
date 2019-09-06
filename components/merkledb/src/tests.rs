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

use exonum_crypto::Hash;
use tempfile::TempDir;

use std::sync::Arc;

use crate::{
    Database, DbOptions, Entry, Fork, KeySetIndex, ListIndex, MapIndex, ProofListIndex,
    ProofMapIndex, RocksDB, SparseListIndex, ValueSetIndex,
};

// This should compile to ensure ?Sized bound on `new_in_family` (see #1024).
#[allow(dead_code, unreachable_code, unused_variables)]
fn should_compile() {
    let fork: Fork = unimplemented!();
    let _: Entry<_, ()> = Entry::new_in_family("", "", &fork);
    let _: KeySetIndex<_, Hash> = KeySetIndex::new_in_family("", "", &fork);
    let _: ListIndex<_, ()> = ListIndex::new_in_family("", "", &fork);
    let _: MapIndex<_, Hash, ()> = MapIndex::new_in_family("", "", &fork);
    let _: ProofListIndex<_, ()> = ProofListIndex::new_in_family("", "", &fork);
    let _: ProofMapIndex<_, Hash, ()> = ProofMapIndex::new_in_family("", "", &fork);
    let _: SparseListIndex<_, ()> = SparseListIndex::new_in_family("", "", &fork);
    let _: ValueSetIndex<_, ()> = ValueSetIndex::new_in_family("", "", &fork);
}

#[test]
fn checkpoints() {
    let src_temp_dir = TempDir::new().unwrap();
    let dst_temp_dir = TempDir::new().unwrap();

    let src_path = src_temp_dir.path().join("src");
    let dst_path = dst_temp_dir.path().join("dst");

    // Convert into `dyn Database` to test downcast.
    let db: Arc<dyn Database> = RocksDB::open(&*src_path, &DbOptions::default())
        .unwrap()
        .into();

    // Write some data to the source database.
    {
        let fork = db.fork();
        Entry::new("first", &fork).set(vec![1_u8; 1024]);
        db.merge_sync(fork.into_patch()).unwrap();
    }

    // Create checkpoint
    {
        let rocks_db = db.downcast_ref::<RocksDB>().unwrap();
        rocks_db.create_checkpoint(&*dst_path).unwrap();
    }

    // Add more data to the source database
    {
        let fork = db.fork();
        Entry::new("second", &fork).set(vec![2_u8; 1024]);
        db.merge_sync(fork.into_patch()).unwrap();
    }

    // Close source database.
    drop(db);

    // Open checkpoint and Assert that it's not affected
    // by the data added after create_checkpoint call.
    {
        let checkpoint = RocksDB::open(&*dst_path, &DbOptions::default()).unwrap();
        let fork = checkpoint.fork();

        assert_eq!(Entry::new("first", &fork).get(), Some(vec![1_u8; 1024]));
        assert_eq!(Entry::new("second", &fork).get(), None::<Vec<u8>>);

        // Add more data to the checkpoint
        Entry::new("third", &fork).set(vec![3_u8; 1024]);
        checkpoint.merge_sync(fork.into_patch()).unwrap();
    }

    // Assert that source database is not affected by the data added to checkpoint.
    {
        let db = RocksDB::open(&*src_path, &DbOptions::default()).unwrap();
        let fork = db.fork();

        assert_eq!(Entry::new("first", &fork).get(), Some(vec![1_u8; 1024]));
        assert_eq!(Entry::new("second", &fork).get(), Some(vec![2_u8; 1024]));
        assert_eq!(Entry::new("third", &fork).get(), None::<Vec<u8>>);
    }

    // Delete source database's directory.
    drop(src_temp_dir);

    // Assert that checkpoint is not affected if source database is deleted.
    {
        let checkpoint = RocksDB::open(&*dst_path, &DbOptions::default()).unwrap();
        let fork = checkpoint.fork();

        assert_eq!(Entry::new("first", &fork).get(), Some(vec![1_u8; 1024]));
        assert_eq!(Entry::new("second", &fork).get(), None::<Vec<u8>>);

        // Add more data to the checkpoint
        Entry::new("third", &fork).set(vec![3_u8; 1024]);
        checkpoint.merge_sync(fork.into_patch()).unwrap();
    }
}
