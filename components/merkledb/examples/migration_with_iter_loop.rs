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

//! Shows how to migrate database data with the `MigrationHelper::iter_loop`. The migration follows the following scenario:
//!
//! 1. We create and fill database with random data according to schema defined in the
//!   `migration::v1` module with the `create_initial_data` method.
//! 2. We create `MigrationHelper` for this database.
//! 3. We perform migration from the `v1` schema to the `v2` schema
//!   with the help of the `create_migration` and `migrate_wallets` methods.
//!   The method transforms the data in the old schema to conform to the new schema.
//!   Wallets migration is performed with `MigrationHelper::iter_loop`.
//!   The old data is **not** removed at this stage; rather, it exists alongside
//!   the migrated data. This is useful in case the migration needs to be reverted for some reason.
//! 4. We complete the migration by calling `flush_migration`. This moves the migrated data
//!   to its intended place and removes the old data marked for removal.

use exonum_merkledb::{
    access::Prefixed,
    migration::{flush_migration, MigrationHelper},
    Database, ObjectHash, Result as DbResult,
};

use std::sync::Arc;

use migration::{
    check_data_after_flush, check_data_after_merge, check_data_before_flush, create_initial_data,
    v1, v2,
};

mod migration;

fn create_migration(helper: &mut MigrationHelper) -> DbResult<()> {
    println!("\nStarted migration");

    {
        let new_data = helper.new_data();
        let old_data = helper.old_data();

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

    // Migrate wallets using `MigrationHelper::iter_loop`:
    // `Wallet::public_key` field will be removed.
    // `Wallet::history_hash` field will be added.
    // Wallets and history from username Eve will be removed.
    migrate_wallets(helper)
}

/// Provides migration of wallets with `MigrationHelper::iter_loop`.
/// `iter_loop` is designed to allow to merge changes to the database from time to time,
/// so we are migrating wallets in chunks here.
fn migrate_wallets(helper: &mut MigrationHelper) -> DbResult<()> {
    helper.iter_loop(|helper, iters| {
        let old_schema = v1::Schema::new(helper.old_data());
        let mut new_schema = v2::Schema::new(helper.new_data());

        const CHUNK_SIZE: usize = 1_000;
        for (public_key, wallet) in iters
            .create("wallets", &old_schema.wallets)
            .take(CHUNK_SIZE)
        {
            if wallet.username == "Eve" {
                // We don't like Eves 'round these parts. Remove her transaction history
                // and don't migrate the wallet.
                helper
                    .new_data()
                    .create_tombstone(("histories", &public_key));
            } else {
                // Merkelize the wallet history.
                let mut history = new_schema.histories.get(&public_key);
                history.extend(&old_schema.histories.get(&public_key));

                let new_wallet = v2::Wallet {
                    username: wallet.username,
                    balance: wallet.balance,
                    history_hash: history.object_hash(),
                };
                new_schema.wallets.put(&public_key, new_wallet);
            }
        }

        println!("Processed chunk of {} wallets", CHUNK_SIZE);
    })
}

fn main() {
    // Creating a temporary DB and filling it with some data.
    let db: Arc<dyn Database> = Arc::new(create_initial_data());

    // Creating helper to perform migration.
    let mut helper = MigrationHelper::new(db.clone(), "test");

    {
        // State before migration.
        let old_schema = v1::Schema::new(helper.old_data());
        println!("Before migration:");
        old_schema.print_wallets();
    }

    // Migrate the data.
    if let Err(err) = create_migration(&mut helper) {
        println!("Migration failed: {}", err);
        return;
    }

    // Call `MigrationHelper::finish` to merge changes to the database.
    if let Err(err) = helper.finish() {
        println!("Migration finish failed: {}", err);
        return;
    }

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
