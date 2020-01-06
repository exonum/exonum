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

//! Shows how to migrate database data. The migration follows the following scenario:
//!
//! 1. We create and fill database with random data according to schema defined in the
//!   `migration::v1` module with the `create_initial_data` method.
//! 2. We perform migration from the `v1` schema to the `v2` schema with the help of the `create_migration` method.
//!   The method transforms the data in the old schema to conform to the new schema.
//!   The old data is **not** removed at this stage; rather, it exists alongside
//!   the migrated data. This is useful in case the migration needs to be reverted for some reason.
//! 3. We complete the migration by calling `flush_migration`. This moves the migrated data
//!   to its intended place and removes the old data marked for removal.

use exonum_merkledb::{
    access::Prefixed,
    migration::{flush_migration, Migration},
    Fork,
};

use migration::*;

mod migration;

fn create_migration(fork: &Fork) {
    println!("\nStarted migration");

    {
        let new_data = Migration::new("test", fork);
        let old_data = Prefixed::new("test", fork.readonly());

        let old_schema = v1::Schema::new(old_data);
        let mut new_schema = v2::Schema::new(new_data);

        // Move `ticker` and `divisibility` to `config`.
        let config = v2::Config {
            ticker: old_schema.ticker.get().unwrap(),
            divisibility: old_schema.divisibility.get().unwrap_or(0),
        };
        new_schema.config.set(config);
        // Mark these two indexes for removal.
        new_data.create_tombstone("ticker");
        new_data.create_tombstone("divisibility");
    }

    {
        // Migrate wallets using schema:
        // `Wallet::public_key` field will be removed.
        // `Wallet::history_hash` field will be added.
        // Wallets and history from username Eve will be removed.
        let new_data = Migration::new("test", fork);
        let old_data = Prefixed::new("test", fork.readonly());
        migrate_wallets_with_schema(new_data, old_data);
    }
}

fn main() {
    // Creating a temporary DB and filling it with some data.
    let db = create_initial_data();

    let fork = db.fork();
    {
        // State before migration.
        let old_data = Prefixed::new("test", fork.readonly());
        let old_schema = v1::Schema::new(old_data.clone());
        println!("Before migration:");
        old_schema.print_wallets();
    }

    // Migrate the data.
    create_migration(&fork);
    // Merge patch with migrated data.
    db.merge(fork.into_patch()).unwrap();

    // At this point the old data and new data are still present in the storage,
    // but new data is in the unmerged form.

    // Check that DB contains old and new data.
    let snapshot = db.snapshot();
    check_data_before_flush(&snapshot);

    // Finalize the migration by calling `flush_migration`.
    let mut fork = db.fork();
    flush_migration(&mut fork, "test");

    // At this point the new indexes have replaced the old ones in the fork.
    // And indexes are aggregated in the default namespace.

    // Check that indexes are updated.
    let patch = fork.into_patch();
    check_data_after_flush(&patch);

    // When the patch is merged, the situation remains the same.
    db.merge(patch).unwrap();

    // Check that data was updated after merge.
    let snapshot = db.snapshot();
    check_data_after_merge(&snapshot);

    // State after migration.
    let schema = v2::Schema::new(Prefixed::new("test", &snapshot));
    println!("After migration:");
    schema.print_wallets();
}
