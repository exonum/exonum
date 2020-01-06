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

use failure::Error;
use rand::{seq::SliceRandom, thread_rng, Rng};
use serde_derive::*;

use std::borrow::Cow;
use std::sync::Arc;

use exonum_crypto::{Hash, PublicKey, HASH_SIZE, PUBLIC_KEY_LENGTH};
use exonum_derive::FromAccess;
use exonum_merkledb::{
    access::{Access, AccessExt, AsReadonly, FromAccess, Prefixed, RawAccess},
    impl_object_hash_for_binary_value,
    migration::{flush_migration, Migration},
    BinaryValue, Database, Entry, Group, ListIndex, MapIndex, ObjectHash, ProofEntry,
    ProofListIndex, ProofMapIndex, Snapshot, SystemSchema, TemporaryDB,
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

/// Creates initial DB with some random data.
fn create_initial_data() -> TemporaryDB {
    let db = TemporaryDB::new();
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

/// Checks that we have old and new data in the storage after migration.
fn check_data_before_flush(snapshot: &dyn Snapshot) {
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

/// Checks that old data was replaced by new data in the storage.
fn check_data_after_flush(view: impl RawAccess + AsReadonly + Copy) {
    let new_schema = v2::Schema::new(Prefixed::new("test", view));
    assert_eq!(new_schema.config.get().unwrap().divisibility, 8);
    assert!(!view.get_entry::<_, u8>("test.divisibility").exists());

    // The indexes are now aggregated in the default namespace.
    let system_schema = SystemSchema::new(view);
    let state = system_schema.state_aggregator();
    assert_eq!(
        state.keys().collect::<Vec<_>>(),
        vec!["test.config", "test.wallets", "unrelated.list"]
    );
}

/// Performs common migration logic.
pub fn perform_migration<F>(migrate: F)
where
    F: FnOnce(Arc<dyn Database>),
{
    // Creating a temporary DB and filling it with some data.
    let db: Arc<dyn Database> = Arc::new(create_initial_data());

    let fork = db.fork();
    {
        // State before migration.
        let old_data = Prefixed::new("test", fork.readonly());
        let old_schema = v1::Schema::new(old_data.clone());
        println!("Before migration:");
        old_schema.print_wallets();
    }

    // Execute data migration logic.
    migrate(Arc::clone(&db));

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
    check_data_after_flush(&snapshot);

    // State after migration.
    let schema = v2::Schema::new(Prefixed::new("test", &snapshot));
    println!("After migration:");
    schema.print_wallets();
}
