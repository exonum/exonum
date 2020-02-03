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

// cspell:ignore Trillian, Vogon

use exonum::{
    crypto::{hash, KeyPair, Seed},
    runtime::{
        migrations::{
            InitMigrationError, LinearMigrations, MigrateData, MigrationContext, MigrationError,
            MigrationScript,
        },
        versioning::Version,
    },
};
use exonum_derive::*;
use exonum_rust_runtime::{Service, ServiceFactory};
use rand::{seq::SliceRandom, thread_rng, Rng};

use std::borrow::Cow;

use exonum_testkit::migrations::{AbortPolicy, MigrationTest, ScriptExt};

#[derive(Debug, Clone)]
struct TestUser {
    full_name: Cow<'static, str>,
    first_name: Cow<'static, str>,
    last_name: Cow<'static, str>,
    balance: u64,
}

const USERS: &[TestUser] = &[
    TestUser {
        full_name: Cow::Borrowed("Deep Thought"),
        first_name: Cow::Borrowed("Deep"),
        last_name: Cow::Borrowed("Thought"),
        balance: 42,
    },
    TestUser {
        full_name: Cow::Borrowed("Arthur Dent"),
        first_name: Cow::Borrowed("Arthur"),
        last_name: Cow::Borrowed("Dent"),
        balance: 7,
    },
    TestUser {
        full_name: Cow::Borrowed("Trillian"),
        first_name: Cow::Borrowed("Trillian"),
        last_name: Cow::Borrowed(""),
        balance: 90,
    },
    TestUser {
        full_name: Cow::Borrowed("Marvin \"The Paranoid\" Android"),
        first_name: Cow::Borrowed("Marvin \"The Paranoid\""),
        last_name: Cow::Borrowed("Android"),
        balance: 0,
    },
];

impl TestUser {
    fn keypair(&self) -> KeyPair {
        let seed = hash(self.full_name.as_bytes());
        let seed = Seed::from_slice(&seed[..]).unwrap();
        KeyPair::from_seed(&seed)
    }
}

/// Initial service schema.
mod v01 {
    use exonum::{
        crypto::PublicKey,
        merkledb::{
            access::{Access, FromAccess, Prefixed},
            Fork, MapIndex,
        },
    };
    use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
    use serde_derive::{Deserialize, Serialize};

    use crate::TestUser;

    #[derive(Debug, Serialize, Deserialize)]
    #[derive(BinaryValue, ObjectHash)]
    #[binary_value(codec = "bincode")]
    pub struct Wallet {
        pub username: String,
        pub balance: u64,
    }

    #[derive(Debug, FromAccess)]
    pub struct Schema<T: Access> {
        pub wallets: MapIndex<T::Base, PublicKey, Wallet>,
    }

    impl<T: Access> Schema<T> {
        pub fn new(access: T) -> Self {
            Self::from_root(access).unwrap()
        }
    }

    pub(crate) fn generate_test_data(access: Prefixed<&Fork>, users: &[TestUser]) {
        let mut schema = Schema::new(access);
        for user in users {
            let key = user.keypair().public_key();
            let wallet = Wallet {
                username: user.full_name.to_string(),
                balance: user.balance,
            };
            schema.wallets.put(&key, wallet);
        }
    }
}

mod v02 {
    use exonum::crypto::PublicKey;
    use exonum::merkledb::{
        access::{Access, FromAccess, Prefixed},
        ProofEntry, ProofMapIndex, Snapshot,
    };
    use exonum_derive::FromAccess;

    use crate::{v01::Wallet, TestUser};

    #[derive(Debug, FromAccess)]
    pub struct Schema<T: Access> {
        pub wallets: ProofMapIndex<T::Base, PublicKey, Wallet>,
        pub total_balance: ProofEntry<T::Base, u64>,
    }

    impl<T: Access> Schema<T> {
        pub fn new(access: T) -> Self {
            Self::from_root(access).unwrap()
        }
    }

    pub(crate) fn verify_schema(snapshot: Prefixed<&dyn Snapshot>, users: &[TestUser]) {
        let schema = Schema::new(snapshot);
        for user in users {
            let key = user.keypair().public_key();
            let wallet = schema.wallets.get(&key).unwrap();
            assert_eq!(wallet.balance, user.balance);
            assert_eq!(wallet.username, user.full_name);
        }
        assert_eq!(schema.wallets.iter().count(), users.len());

        let total_balance = schema.total_balance.get().unwrap();
        assert_eq!(
            total_balance,
            users.iter().map(|user| user.balance).sum::<u64>()
        );
    }
}

mod v05 {
    use exonum::crypto::PublicKey;
    use exonum::merkledb::{
        access::{Access, AccessExt, FromAccess, Prefixed},
        ProofEntry, ProofMapIndex, Snapshot,
    };
    use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
    use serde_derive::{Deserialize, Serialize};

    use crate::TestUser;

    #[derive(Debug, Serialize, Deserialize)]
    #[derive(BinaryValue, ObjectHash)]
    #[binary_value(codec = "bincode")]
    pub struct Wallet {
        pub first_name: String,
        pub last_name: String,
        pub balance: u64,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[derive(BinaryValue, ObjectHash)]
    #[binary_value(codec = "bincode")]
    pub struct Summary {
        pub ticker: String,
        pub total_balance: u64,
    }

    #[derive(Debug, FromAccess)]
    pub struct Schema<T: Access> {
        pub wallets: ProofMapIndex<T::Base, PublicKey, Wallet>,
        pub summary: ProofEntry<T::Base, Summary>,
    }

    impl<T: Access> Schema<T> {
        pub fn new(access: T) -> Self {
            Self::from_root(access).unwrap()
        }
    }

    pub(crate) fn verify_schema(snapshot: Prefixed<&dyn Snapshot>, users: &[TestUser]) {
        let schema = Schema::new(snapshot.clone());
        for user in users {
            let key = user.keypair().public_key();
            let wallet = schema.wallets.get(&key).unwrap();
            assert_eq!(wallet.balance, user.balance);
            assert_eq!(wallet.first_name, user.first_name);
            assert_eq!(wallet.last_name, user.last_name);
        }
        assert_eq!(schema.wallets.iter().count(), users.len());

        let summary = schema.summary.get().unwrap();
        assert_eq!(summary.ticker, "test");
        assert_eq!(
            summary.total_balance,
            users.iter().map(|user| user.balance).sum::<u64>()
        );

        // Check that the outdated index has been deleted.
        assert_eq!(snapshot.index_type("total_balance"), None);
    }
}

/// First migration script. Merkelizes the wallets table and records the total number of tokens.
fn merkelize_wallets(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    let old_schema = v01::Schema::new(ctx.helper.old_data());
    let mut new_schema = v02::Schema::new(ctx.helper.new_data());

    let mut total_balance = 0;
    for (key, wallet) in &old_schema.wallets {
        total_balance += wallet.balance;
        new_schema.wallets.put(&key, wallet);
    }
    new_schema.total_balance.set(total_balance);
    Ok(())
}

/// The alternative version of the previous migration script, which uses database merges.
fn merkelize_wallets_with_merges(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    const CHUNK_SIZE: usize = 500;

    ctx.helper.iter_loop(|helper, iters| {
        let old_schema = v01::Schema::new(helper.old_data());
        let mut new_schema = v02::Schema::new(helper.new_data());

        let iter = iters.create("wallets", &old_schema.wallets);
        let mut total_balance = 0;
        for (key, wallet) in iter.take(CHUNK_SIZE) {
            total_balance += wallet.balance;
            new_schema.wallets.put(&key, wallet);
        }
        let prev_balance = new_schema.total_balance.get().unwrap_or(0);
        new_schema.total_balance.set(prev_balance + total_balance);
    })?;
    Ok(())
}

/// Second migration script. Transforms the wallet type and reorganizes the service summary.
fn transform_wallet_type(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    let old_schema = v02::Schema::new(ctx.helper.old_data());
    let mut new_schema = v05::Schema::new(ctx.helper.new_data());

    let total_balance = old_schema.total_balance.get().unwrap_or(0);
    new_schema.summary.set(v05::Summary {
        ticker: ctx.instance_spec.name.clone(),
        total_balance,
    });
    ctx.helper.new_data().create_tombstone("total_balance");

    for (key, wallet) in &old_schema.wallets {
        let name_parts: Vec<_> = wallet.username.rsplitn(2, ' ').collect();
        let (first_name, last_name) = match &name_parts[..] {
            [first_name] => (*first_name, ""),
            [last_name, first_name] => (*first_name, *last_name),
            _ => unreachable!(),
        };
        let new_wallet = v05::Wallet {
            first_name: first_name.to_owned(),
            last_name: last_name.to_owned(),
            balance: wallet.balance,
        };
        new_schema.wallets.put(&key, new_wallet);
    }
    Ok(())
}

/// Incorrect version of `merkelize_wallets_with_merges`.
fn merkelize_wallets_incorrect(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    const CHUNK_SIZE: usize = 500;

    // Moving the balance initialization outside of the loop is an error! Indeed,
    // if the script is restarted, the accumulated balance is forgotten.
    let mut total_balance = 0;

    ctx.helper.iter_loop(|helper, iters| {
        let old_schema = v01::Schema::new(helper.old_data());
        let mut new_schema = v02::Schema::new(helper.new_data());

        let iter = iters.create("wallets", &old_schema.wallets);
        for (key, wallet) in iter.take(CHUNK_SIZE) {
            total_balance += wallet.balance;
            new_schema.wallets.put(&key, wallet);
        }
    })?;

    let mut new_schema = v02::Schema::new(ctx.helper.new_data());
    new_schema.total_balance.set(total_balance);
    Ok(())
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_factory(artifact_name = "exonum.test.Migration", artifact_version = "0.6.2")]
struct MigratedService;

impl Service for MigratedService {}

impl MigrateData for MigratedService {
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        LinearMigrations::new(self.artifact_id().version)
            .add_script(Version::new(0, 2, 0), merkelize_wallets)
            .add_script(Version::new(0, 5, 0), transform_wallet_type)
            .select(start_version)
    }
}

#[test]
fn migration_with_two_scripts() {
    let mut test = MigrationTest::new(MigratedService, Version::new(0, 1, 0));
    let snapshot = test
        .setup(|fork| v01::generate_test_data(fork, USERS))
        .migrate()
        .end_snapshot();
    v05::verify_schema(snapshot, USERS);
}

fn generate_users(rng: &mut impl Rng, user_count: usize) -> Vec<TestUser> {
    (0..user_count)
        .map(|i| {
            let first_name = ["Mouse", "Vogon"].choose(rng).unwrap().to_string();
            let last_name = format!("#{}", i);
            TestUser {
                full_name: format!("{} {}", first_name, last_name).into(),
                first_name: first_name.into(),
                last_name: last_name.into(),
                balance: rng.gen_range(0, 10_000),
            }
        })
        .collect()
}

#[test]
fn migration_with_large_data() {
    const USER_COUNT: usize = 1_234;

    let mut rng = thread_rng();
    let users = generate_users(&mut rng, USER_COUNT);

    let mut test = MigrationTest::new(MigratedService, Version::new(0, 1, 0));
    let snapshot = test
        .setup(|fork| v01::generate_test_data(fork, &users))
        .migrate()
        .end_snapshot();
    v05::verify_schema(snapshot, &users);
}

#[test]
fn migration_with_large_data_and_merges() {
    const USER_COUNT: usize = 3_456;

    let mut rng = thread_rng();
    let users = generate_users(&mut rng, USER_COUNT);

    let mut test = MigrationTest::new(MigratedService, Version::new(0, 1, 0));
    let snapshot = test
        .setup(|fork| v01::generate_test_data(fork, &users))
        .execute_script(merkelize_wallets_with_merges.with_end_version("0.2.0"))
        .end_snapshot();
    v02::verify_schema(snapshot, &users);
}

#[test]
fn migration_testing_detecting_fault_tolerance_error() {
    const USER_COUNT: usize = 2_345;

    let mut rng = thread_rng();
    let users = generate_users(&mut rng, USER_COUNT);

    let mut test = MigrationTest::new(MigratedService, Version::new(0, 1, 0));
    let snapshot = test
        .setup(|fork| v01::generate_test_data(fork, &users))
        .execute_until_flush(
            || merkelize_wallets_incorrect.with_end_version("0.2.0"),
            AbortPolicy::abort_repeatedly(),
        )
        .end_snapshot();

    let schema = v02::Schema::new(snapshot);
    let total_balance = schema.total_balance.get().unwrap();
    let expected_balance = users.iter().map(|user| user.balance).sum::<u64>();
    assert!(total_balance > 0);
    assert!(total_balance < expected_balance); // We've forgotten about ~80% of account balances!
}
