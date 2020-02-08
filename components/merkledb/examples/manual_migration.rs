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

//! This example shows how to provide database data migration manually.
//!
//! The main logic is described in the `manual_migration` and `migrate_wallets` functions.
//!
//! The main points of this example are:
//!
//! - We manually create a `Fork` from the DB, as well as `Migration` and `Prefixed` access
//!   to the data.
//! - We manually apply the resulting `Patch` to the DB.
//!
//! For the description of the common migration scenario, see the `migration` module docs.

use exonum_merkledb::{
    access::Prefixed, migration::Migration, Database, Fork, ObjectHash, ReadonlyFork,
};

use std::sync::Arc;

mod migration;
use crate::migration::{perform_migration, v1, v2};

/// Provides migration of wallets with schema.
///
/// - `Wallet.public_key` field is removed.
/// - `Wallet.history_hash` field is added.
/// - Wallets and wallet history belonging to the users named "Eve' are dropped.
fn migrate_wallets(new_data: Migration<&Fork>, old_data: Prefixed<ReadonlyFork>) {
    let old_schema = v1::Schema::new(old_data);
    let mut new_schema = v2::Schema::new(new_data.clone());

    // Migrate wallets.
    for (i, (public_key, wallet)) in old_schema.wallets.iter().enumerate() {
        if wallet.username == "Eve" {
            // We don't like Eves 'round these parts. Remove her transaction history
            // and don't migrate the wallet.
            new_data.create_tombstone(("histories", &public_key));
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

fn manual_migration(db: Arc<dyn Database>) {
    // Create fork to apply changes to it.
    let fork = db.fork();

    {
        let new_data = Migration::new("test", &fork);
        let mut new_schema = v2::Schema::new(new_data.clone());
        let old_data = Prefixed::new("test", fork.readonly());
        let old_schema = v1::Schema::new(old_data);

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

    let new_data = Migration::new("test", &fork);
    let old_data = Prefixed::new("test", fork.readonly());
    migrate_wallets(new_data, old_data);

    // Merge patch with migrated data.
    db.merge(fork.into_patch()).unwrap();
}

fn main() {
    perform_migration(manual_migration);
}
