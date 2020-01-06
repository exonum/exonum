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
//! 1. We define the old data schema in the `v1` module and fill the database with some
//!   random data.
//! 2. We define the new data schema in the `v2` module.
//! 3. We perform migration of the old data with the help of the `create_migration` method.
//!   The method transforms the data in the old schema to conform to the new schema.
//!   The old data is **not** removed at this stage; rather, it exists alongside
//!   the migrated data. This is useful in case the migration needs to be reverted for some reason.
//! 4. We complete the migration by calling `Fork::flush_migration`. This moves the migrated data
//!   to its intended place and removes the old data marked for removal.

use failure::Error;
use rand::{seq::SliceRandom, thread_rng, Rng};
use serde_derive::*;
use std::sync::Arc;

use std::borrow::Cow;

use exonum_crypto::{Hash, PublicKey, HASH_SIZE, PUBLIC_KEY_LENGTH};
use exonum_derive::FromAccess;
use exonum_merkledb::{
    access::{Access, AccessExt, FromAccess, Prefixed},
    impl_object_hash_for_binary_value,
    migration::{flush_migration, Migration, MigrationHelper},
    BinaryValue, Database, Entry, Fork, Group, ListIndex, MapIndex, ObjectHash, Patch, ProofEntry,
    ProofListIndex, ProofMapIndex, ReadonlyFork, Result as MdbResult, Snapshot, SystemSchema,
    TemporaryDB,
};

const USER_COUNT: usize = 10_000;

pub mod v1 {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Wallet {
        pub public_key: PublicKey, // << removed in `v2`
        pub username: String,
        pub balance: u32,
    }

    impl BinaryValue for Wallet {
        fn to_bytes(&self) -> Vec<u8> {
            bincode::serialize(self).unwrap()
        }

        fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, Error> {
            bincode::deserialize(bytes.as_ref()).map_err(From::from)
        }
    }

    #[derive(Debug, FromAccess)]
    pub struct Schema<T: Access> {
        pub ticker: Entry<T::Base, String>,
        pub divisibility: Entry<T::Base, u8>,
        pub wallets: MapIndex<T::Base, PublicKey, Wallet>,
        pub histories: Group<T, PublicKey, ListIndex<T::Base, Hash>>,
    }

    impl<T: Access> Schema<T> {
        pub fn new(access: T) -> Self {
            Self::from_root(access).unwrap()
        }

        pub fn print_wallets(&self) {
            for (public_key, wallet) in self.wallets.iter().take(10) {
                println!("Wallet[{:?}] = {:?}", public_key, wallet);
                println!(
                    "History = {:?}",
                    self.histories.get(&public_key).iter().collect::<Vec<_>>()
                );
            }
        }
    }
}

pub fn create_initial_data() -> Arc<dyn Database> {
    let db: Arc<dyn Database> = Arc::new(TemporaryDB::new());
    let fork = db.fork();

    {
        const NAMES: &[&str] = &["Alice", "Bob", "Carol", "Dave", "Eve"];

        let mut schema = v1::Schema::new(Prefixed::new("test", &fork));
        schema.ticker.set("XNM".to_owned());
        schema.divisibility.set(8);

        let mut rng = thread_rng();
        for _ in 0..USER_COUNT {
            let mut bytes = [0_u8; PUBLIC_KEY_LENGTH];
            rng.fill(&mut bytes[..]);
            let public_key = PublicKey::new(bytes);
            let username = NAMES.choose(&mut rng).unwrap().to_string();
            let wallet = v1::Wallet {
                public_key,
                username,
                balance: rng.gen_range(0, 1_000),
            };
            schema.wallets.put(&public_key, wallet);

            let history_len = rng.gen_range(0, 10);
            schema
                .histories
                .get(&public_key)
                .extend((0..history_len).map(|_| {
                    let mut bytes = [0_u8; HASH_SIZE];
                    rng.fill(&mut bytes[..]);
                    Hash::new(bytes)
                }));
        }
    }

    fork.get_proof_list("unrelated.list").extend(vec![1, 2, 3]);
    db.merge(fork.into_patch()).unwrap();

    db
}

pub mod v2 {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Wallet {
        pub username: String,
        pub balance: u32,
        pub history_hash: Hash, // << new field
    }

    impl BinaryValue for Wallet {
        fn to_bytes(&self) -> Vec<u8> {
            bincode::serialize(self).unwrap()
        }

        fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, Error> {
            bincode::deserialize(bytes.as_ref()).map_err(From::from)
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Config {
        pub ticker: String,
        pub divisibility: u8,
    }

    impl BinaryValue for Config {
        fn to_bytes(&self) -> Vec<u8> {
            bincode::serialize(self).unwrap()
        }

        fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, Error> {
            bincode::deserialize(bytes.as_ref()).map_err(From::from)
        }
    }

    impl_object_hash_for_binary_value! { Wallet, Config }

    #[derive(Debug, FromAccess)]
    pub struct Schema<T: Access> {
        pub config: ProofEntry<T::Base, Config>,
        pub wallets: ProofMapIndex<T::Base, PublicKey, Wallet>,
        pub histories: Group<T, PublicKey, ProofListIndex<T::Base, Hash>>,
    }

    impl<T: Access> Schema<T> {
        pub fn new(access: T) -> Self {
            Self::from_root(access).unwrap()
        }

        pub fn print_wallets(&self) {
            for (public_key, wallet) in self.wallets.iter().take(10) {
                println!("Wallet[{:?}] = {:?}", public_key, wallet);
                println!(
                    "History = {:?}",
                    self.histories.get(&public_key).iter().collect::<Vec<_>>()
                );
            }
        }
    }
}

/// Provides migration of wallets with schema.
pub fn migrate_wallets_with_schema(new_data: Migration<&Fork>, old_data: Prefixed<ReadonlyFork>) {
    let old_schema = v1::Schema::new(old_data);
    let mut new_schema = v2::Schema::new(new_data);

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

/// Provides migration of wallets with `MigrationHelper::iter_loop`.
/// `iter_loop` is designed to allow to merge changes to the database from time to time,
/// so we are migrating wallets in chunks here.
pub fn migrate_wallets_with_iter_loop(helper: &mut MigrationHelper) -> DbResult<()> {
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

pub fn check_data_before_flush(snapshot: &Box<dyn Snapshot>) {
    let old_schema = v1::Schema::new(Prefixed::new("test", snapshot));
    assert_eq!(old_schema.ticker.get().unwrap(), "XNM");
    // The new data is present, too, in the unmerged form.
    let new_schema = v2::Schema::new(Migration::new("test", snapshot));
    assert_eq!(new_schema.config.get().unwrap().ticker, "XNM");

    let system_schema = SystemSchema::new(snapshot);
    let state = system_schema.state_aggregator();
    assert_eq!(state.keys().collect::<Vec<_>>(), vec!["unrelated.list"]);
    let migration_view = Migration::new("test", snapshot);
    let state = migration_view.state_aggregator();
    assert_eq!(
        state.keys().collect::<Vec<_>>(),
        vec!["test.config", "test.wallets"]
    );
    let new_state_hash = state.object_hash();
    assert_eq!(new_state_hash, migration_view.state_hash());
}

pub fn check_data_after_flush(patch: &Patch) {
    // Now, the new indexes have replaced the old ones.
    let new_schema = v2::Schema::new(Prefixed::new("test", patch));
    assert_eq!(new_schema.config.get().unwrap().divisibility, 8);
    assert!(!patch.get_entry::<_, u8>("test.divisibility").exists());

    // The indexes are now aggregated in the default namespace.
    let system_schema = SystemSchema::new(patch);
    let state = system_schema.state_aggregator();
    assert_eq!(
        state.keys().collect::<Vec<_>>(),
        vec!["test.config", "test.wallets", "unrelated.list"]
    );
}

pub fn check_data_after_merge(snapshot: &Box<dyn Snapshot>) {
    let system_schema = SystemSchema::new(snapshot);
    let state = system_schema.state_aggregator();
    assert_eq!(
        state.keys().collect::<Vec<_>>(),
        vec!["test.config", "test.wallets", "unrelated.list"]
    );
}

fn check_and_finalize_migration(db: Arc<dyn Database>) {
    // For now, the old data is still present in the storage.
    let snapshot = db.snapshot();
    let old_schema = v1::Schema::new(Prefixed::new("test", &snapshot));
    assert_eq!(old_schema.ticker.get().unwrap(), "XNM");
    // The new data is present, too, in the unmerged form.
    let new_schema = v2::Schema::new(Migration::new("test", &snapshot));
    assert_eq!(new_schema.config.get().unwrap().ticker, "XNM");

    let system_schema = SystemSchema::new(&snapshot);
    let state = system_schema.state_aggregator();
    assert_eq!(state.keys().collect::<Vec<_>>(), vec!["unrelated.list"]);
    let migration_view = Migration::new("test", &snapshot);
    let state = migration_view.state_aggregator();
    assert_eq!(
        state.keys().collect::<Vec<_>>(),
        vec!["test.config", "test.wallets"]
    );
    let new_state_hash = state.object_hash();
    assert_eq!(new_state_hash, migration_view.state_hash());

    let mut fork = db.fork();
    flush_migration(&mut fork, "test");
    let patch = fork.into_patch();

    // Now, the new indexes have replaced the old ones.
    let new_schema = v2::Schema::new(Prefixed::new("test", &patch));
    assert_eq!(new_schema.config.get().unwrap().divisibility, 8);
    assert!(!patch.get_entry::<_, u8>("test.divisibility").exists());

    // The indexes are now aggregated in the default namespace.
    let system_schema = SystemSchema::new(&patch);
    let state = system_schema.state_aggregator();
    assert_eq!(
        state.keys().collect::<Vec<_>>(),
        vec!["test.config", "test.wallets", "unrelated.list"]
    );

    // When the patch is merged, the situation remains the same.
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    let schema = v2::Schema::new(Prefixed::new("test", &snapshot));
    println!("\nAfter migration:");
    schema.print_wallets();

    let system_schema = SystemSchema::new(&snapshot);
    let state = system_schema.state_aggregator();
    assert_eq!(
        state.keys().collect::<Vec<_>>(),
        vec!["test.config", "test.wallets", "unrelated.list"]
    );
}

fn create_basic_migration(fork: &Fork) {
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
        let new_data = Migration::new("test", fork);
        let old_data = Prefixed::new("test", fork.readonly());
        migrate_wallets_with_schema(new_data, old_data);
    }
}

/// Example of migration without `MigrationHelper`.
fn basic_migration() {
    println!("\n\nBasic migration.");

    let db: Arc<dyn Database> = Arc::new(TemporaryDB::new());
    let fork = db.fork();
    create_initial_data(&fork);
    fork.get_proof_list("unrelated.list").extend(vec![1, 2, 3]);
    db.merge(fork.into_patch()).unwrap();

    let fork = db.fork();
    let old_data = Prefixed::new("test", fork.readonly());
    {
        let old_schema = v1::Schema::new(old_data.clone());
        println!("Before migration:");
        old_schema.print_wallets();
    }
    create_basic_migration(&fork);
    db.merge(fork.into_patch()).unwrap();

    check_and_finalize_migration(db.clone());
}

fn create_migration_with_helper(
    helper: &mut MigrationHelper,
    migrate_wallets_iter_loop: bool,
) -> MdbResult<()> {
    println!("\nStarted migration.");

    {
        let old_data = helper.old_data();
        let new_data = helper.new_data();

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

    if migrate_wallets_iter_loop {
        migrate_wallets_with_iter_loop(helper)?;
    } else {
        migrate_wallets_with_schema(helper.new_data(), helper.old_data());
    }

    Ok(())
}

/// Example of migration with `MigrationHelper`.
fn migration_with_helper() {
    println!("\n\nMigration with MigrationHelper.");

    let db: Arc<dyn Database> = Arc::new(TemporaryDB::new());
    let fork = db.fork();
    create_initial_data(&fork);
    fork.get_proof_list("unrelated.list").extend(vec![1, 2, 3]);
    db.merge(fork.into_patch()).unwrap();

    let mut helper = MigrationHelper::new(db.clone(), "test");
    {
        let old_schema = v1::Schema::new(helper.old_data().clone());
        println!("Before migration:");
        old_schema.print_wallets();
    }

    if let Err(err) = create_migration_with_helper(&mut helper, false) {
        println!("Migration failed: {}", err);
        return;
    }

    let new_state = helper.finish();
    if let Err(err) = new_state {
        println!("Migration finish failed: {}", err);
        return;
    }

    check_and_finalize_migration(db.clone());
}

/// Example of migration with `MigrationHelper::iter_loop`.
fn migration_with_helper_iter_loop() {
    println!("\n\nMigration with MigrationHelper::iter_loop.");

    let db: Arc<dyn Database> = Arc::new(TemporaryDB::new());
    let fork = db.fork();
    create_initial_data(&fork);
    fork.get_proof_list("unrelated.list").extend(vec![1, 2, 3]);
    db.merge(fork.into_patch()).unwrap();

    let mut helper = MigrationHelper::new(db.clone(), "test");
    {
        let old_schema = v1::Schema::new(helper.old_data().clone());
        println!("Before migration:");
        old_schema.print_wallets();
    }

    if let Err(err) = create_migration_with_helper(&mut helper, true) {
        println!("Migration failed: {}", err);
        return;
    }

    let new_state = helper.finish();
    if let Err(err) = new_state {
        println!("Failed to finish migration: {}", err);
        return;
    }

    check_and_finalize_migration(db.clone());
}

fn main() {
    basic_migration();
    migration_with_helper();
    migration_with_helper_iter_loop();
}
