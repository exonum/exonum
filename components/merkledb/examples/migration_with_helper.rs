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

//! Shows how to migrate database data with the `MigrationHelper`. The migration follows the following scenario:
//!
//! 1. We create and fill database with random data according to schema defined in the
//!   `migration::v1` module with the `create_initial_data` method.
//! 2. We create `MigrationHelper` for this database.
//! 3. We perform migration from the `v1` schema to the `v2` schema
//!   with the help of the `create_migration` and `migrate_wallets` methods.
//!   The method transforms the data in the old schema to conform to the new schema.
//!   The old data is **not** removed at this stage; rather, it exists alongside
//!   the migrated data. This is useful in case the migration needs to be reverted for some reason.
//! 4. We complete the migration by calling `flush_migration`. This moves the migrated data
//!   to its intended place and removes the old data marked for removal.

use exonum_merkledb::{migration::MigrationHelper, Database, ObjectHash};

use std::sync::Arc;

use migration::{perform_migration, v1, v2};

mod migration;

/// Provides migration of wallets with schema and `MigrationHelper`.
fn migrate_wallets(helper: &MigrationHelper) {
    let old_schema = v1::Schema::new(helper.old_data());
    let mut new_schema = v2::Schema::new(helper.new_data());

    // Migrate wallets.
    for (i, (public_key, wallet)) in old_schema.wallets.iter().enumerate() {
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

        if i % 1_000 == 999 {
            println!("Processed {} wallets", i + 1);
        }
    }
}

fn migration_with_helper(db: Arc<dyn Database>) {
    // Creating helper to perform migration.
    let helper = MigrationHelper::new(db.clone(), "test");

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

    // Migrate wallets using schema:
    // `Wallet::public_key` field will be removed.
    // `Wallet::history_hash` field will be added.
    // Wallets and history from username Eve will be removed.
    migrate_wallets(&helper);

    // Call `MigrationHelper::finish` to merge changes to the database.
    helper.finish().expect("Migration finish failed.");
}

fn main() {
    perform_migration(migration_with_helper);
}
