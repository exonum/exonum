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

use exonum_merkledb::{access::CopyAccessExt, Database, DbOptions, RocksDB};
use tempfile::TempDir;

#[test]
fn checkpoints() {
    let src_temp_dir = TempDir::new().unwrap();
    let dst_temp_dir = TempDir::new().unwrap();

    let src_path = src_temp_dir.path().join("src");
    let dst_path = dst_temp_dir.path().join("dst");

    // Convert into `dyn Database` to test downcast.
    let db = RocksDB::open(&*src_path, &DbOptions::default()).unwrap();

    // Write some data to the source database.
    {
        let fork = db.fork();
        fork.get_entry("first").set(vec![1_u8; 1024]);
        db.merge_sync(fork.into_patch()).unwrap();
    }

    // Create checkpoint
    {
        db.create_checkpoint(&*dst_path).unwrap();
    }

    // Add more data to the source database
    {
        let fork = db.fork();
        fork.get_entry("second").set(vec![2_u8; 1024]);
        db.merge_sync(fork.into_patch()).unwrap();
    }

    // Close source database.
    drop(db);

    // Open checkpoint and Assert that it's not affected
    // by the data added after create_checkpoint call.
    {
        let checkpoint = RocksDB::open(&*dst_path, &DbOptions::default()).unwrap();
        let fork = checkpoint.fork();

        assert_eq!(fork.get_entry("first").get(), Some(vec![1_u8; 1024]));
        assert_eq!(fork.get_entry("second").get(), None::<Vec<u8>>);

        // Add more data to the checkpoint
        fork.get_entry("third").set(vec![3_u8; 1024]);
        checkpoint.merge_sync(fork.into_patch()).unwrap();
    }

    // Assert that source database is not affected by the data added to checkpoint.
    {
        let db = RocksDB::open(&*src_path, &DbOptions::default()).unwrap();
        let fork = db.fork();

        assert_eq!(fork.get_entry("first").get(), Some(vec![1_u8; 1024]));
        assert_eq!(fork.get_entry("second").get(), Some(vec![2_u8; 1024]));
        assert_eq!(fork.get_entry("third").get(), None::<Vec<u8>>);
    }

    // Delete source database's directory.
    drop(src_temp_dir);

    // Assert that checkpoint is not affected if source database is deleted.
    {
        let checkpoint = RocksDB::open(&*dst_path, &DbOptions::default()).unwrap();
        let fork = checkpoint.fork();

        assert_eq!(fork.get_entry("first").get(), Some(vec![1_u8; 1024]));
        assert_eq!(fork.get_entry("second").get(), None::<Vec<u8>>);

        // Add more data to the checkpoint
        fork.get_entry("third").set(vec![3_u8; 1024]);
        checkpoint.merge_sync(fork.into_patch()).unwrap();
    }
}
