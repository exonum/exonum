//! Shows how to migrate database data.

use failure::Error;
use rand::{seq::SliceRandom, thread_rng, Rng};
use serde_derive::*;

use std::borrow::Cow;

use exonum_crypto::{Hash, PublicKey, HASH_SIZE, PUBLIC_KEY_LENGTH};
use exonum_derive::FromAccess;
use exonum_merkledb::{
    access::{Access, AccessExt, Prefixed},
    impl_object_hash_for_binary_value, BinaryValue, Database, Entry, Fork, Group, ListIndex,
    MapIndex, ObjectHash, ProofEntry, ProofListIndex, ProofMapIndex, ReadonlyFork, SystemSchema,
    TemporaryDB,
};

const USER_COUNT: usize = 10_000;

mod v1 {
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

fn create_initial_data(fork: &Fork) {
    const NAMES: &[&str] = &["Alice", "Bob", "Carol", "Dave", "Eve"];

    let mut schema = v1::Schema::new(Prefixed::new("test", fork));
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

mod v2 {
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

fn migrate(new_data: Prefixed<&Fork>, old_data: Prefixed<ReadonlyFork>) {
    println!("\nStarted migration");
    let old_schema = v1::Schema::new(old_data);
    let mut new_schema = v2::Schema::new(new_data.clone());

    // Move `ticker` and `divisibility` to `config`.
    let config = v2::Config {
        ticker: old_schema.ticker.get().unwrap(),
        divisibility: old_schema.divisibility.get().unwrap_or(0),
    };
    new_schema.config.set(config);
    // Mark these two indexes for removal.
    new_data.clone().create_tombstone("ticker");
    new_data.clone().create_tombstone("divisibility");

    // Migrate wallets.
    for (i, (public_key, wallet)) in old_schema.wallets.iter().enumerate() {
        if wallet.username == "Eve" {
            // We don't like Eves 'round these parts. Remove her transaction history
            // and don't migrate the wallet.
            new_data
                .clone()
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

fn main() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    create_initial_data(&fork);
    fork.get_proof_list("unrelated.list").extend(vec![1, 2, 3]);
    db.merge(fork.into_patch()).unwrap();

    let fork = db.fork();
    let new_data = Prefixed::for_migration("test", &fork);
    let old_data = Prefixed::new("test", fork.readonly());
    {
        let old_schema = v1::Schema::new(old_data.clone());
        println!("Before migration:");
        old_schema.print_wallets();
    }
    migrate(new_data, old_data);
    db.merge(fork.into_patch()).unwrap();

    // For now, the old data is still present in the storage.
    let snapshot = db.snapshot();
    let old_schema = v1::Schema::new(Prefixed::new("test", &snapshot));
    assert_eq!(old_schema.ticker.get().unwrap(), "XNM");
    // The new data is present, too, in the unmerged form.
    let new_schema = v2::Schema::new(Prefixed::for_migration("test", &snapshot));
    assert_eq!(new_schema.config.get().unwrap().ticker, "XNM");

    let system_schema = SystemSchema::new(&snapshot);
    let state = system_schema.state_aggregator();
    assert_eq!(state.keys().collect::<Vec<_>>(), vec!["unrelated.list"]);
    let state = system_schema.namespace_state_aggregator("test");
    assert_eq!(
        state.keys().collect::<Vec<_>>(),
        vec!["test.config", "test.wallets"]
    );
    let new_state_hash = state.object_hash();
    assert_eq!(new_state_hash, system_schema.namespace_state_hash("test"));

    let mut fork = db.fork();
    fork.flush_migration("test");
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
