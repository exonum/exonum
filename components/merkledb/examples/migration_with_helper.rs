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

//! This example shows how to provide database data migration with the `MigrationHelper`.
//! `MigrationHelper` provides methods to get access to the old
//! and new versions of the data, and to merge changes, so we don't need to do it manually.
//!
//! The main logic is described in the `migration_with_helper` and `migrate_wallets` functions.
//!
//! The main points of this example are:
//!
//! - We create `MigrationHelper` for the DB that allows us to access
//!   old and new data in a structured way.
//! - We use `MigrationHelper::finish()` to merge the changes to the database.
//!
//! For the description of the common migration scenario, see the `migration` module docs.

use exonum_merkledb::{migration::MigrationHelper, Database, ObjectHash};

use std::sync::Arc;

use crate::migration::{perform_migration, v1, v2};
mod migration;

/// Migrates wallets using `MigrationHelper`.
///
/// - `Wallet.public_key` field is removed.
/// - `Wallet.history_hash` field is added.
/// - Wallets and wallet history belonging to the users named "Eve' are dropped.
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
        let old_data = helper.old_data();
        let old_schema = v1::Schema::new(old_data);
        let new_data = helper.new_data();
        let mut new_schema = v2::Schema::new(new_data.clone());

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

    migrate_wallets(&helper);
    // Call `MigrationHelper::finish` to merge changes to the database.
    helper.finish().expect("Migration finish failed.");
}

fn main() {
    perform_migration(migration_with_helper);
}
