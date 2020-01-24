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

// This is adapted version of service with migrations from `testkit/tests/migrations/mod.rs`.
//
// While version in `testkit` is designed for testing the migration process itself,
// this version is designed to test migration workflow in supervisor service.
//
// As a result, this version is somewhat simplified and does not test big/random data sets,
// but focuses more on different migration scenarios.

// cspell:ignore Trillian, Vogon

use exonum::{
    crypto::{gen_keypair_from_seed, hash, PublicKey, SecretKey, Seed},
    runtime::{
        migrations::{
            InitMigrationError, LinearMigrations, MigrateData, MigrationContext, MigrationError,
            MigrationScript,
        },
        versioning::Version,
        ExecutionContext, ExecutionError, InstanceId,
    },
};
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, Service, ServiceFactory};
// use rand::{seq::SliceRandom, thread_rng, Rng};

use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct TestUser {
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
    fn keypair(&self) -> (PublicKey, SecretKey) {
        let seed = hash(self.full_name.as_bytes());
        let seed = Seed::from_slice(&seed[..]).unwrap();
        gen_keypair_from_seed(&seed)
    }
}

/// Initial service schema.
pub(super) mod v01 {
    use exonum::{
        crypto::PublicKey,
        merkledb::{
            access::{Access, FromAccess, Prefixed},
            MapIndex, Snapshot,
        },
    };
    use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
    use serde_derive::{Deserialize, Serialize};

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

    #[allow(clippy::borrowed_box)] // We can't just convert `&Box<dyn Snapshot>` to `&dyn Snapshot`.
    pub(crate) fn verify_schema(snapshot: Prefixed<'_, &Box<dyn Snapshot>>) {
        let users = super::USERS;

        let schema = Schema::new(snapshot.clone());
        for user in users {
            let (key, _) = user.keypair();
            let wallet = schema
                .wallets
                .get(&key)
                .expect("V01: User wallet not found");
            assert_eq!(wallet.username, user.full_name.to_string());
            assert_eq!(wallet.balance, user.balance);
        }
    }
}

pub(super) mod v02 {
    use exonum::crypto::PublicKey;
    use exonum::merkledb::{
        access::{Access, FromAccess, Prefixed},
        ProofEntry, ProofMapIndex, Snapshot,
    };
    use exonum_derive::FromAccess;

    use super::v01::Wallet;

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

    #[allow(clippy::borrowed_box)] // We can't just convert `&Box<dyn Snapshot>` to `&dyn Snapshot`.
    pub(crate) fn verify_schema(snapshot: Prefixed<'_, &Box<dyn Snapshot>>) {
        let users = super::USERS;
        let schema = Schema::new(snapshot);
        for user in users {
            let (key, _) = user.keypair();
            let wallet = schema
                .wallets
                .get(&key)
                .expect("V02: User wallet not found");
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

pub(super) mod v05 {
    use exonum::crypto::PublicKey;
    use exonum::merkledb::{
        access::{Access, AccessExt, FromAccess, Prefixed},
        ProofEntry, ProofMapIndex, Snapshot,
    };
    use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
    use serde_derive::{Deserialize, Serialize};

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

    #[allow(clippy::borrowed_box)] // We can't just convert `&Box<dyn Snapshot>` to `&dyn Snapshot`.
    pub(crate) fn verify_schema(snapshot: Prefixed<'_, &Box<dyn Snapshot>>) {
        let users = super::USERS;

        let schema = Schema::new(snapshot.clone());
        for user in users {
            let (key, _) = user.keypair();
            let wallet = schema
                .wallets
                .get(&key)
                .expect("V05: User wallet not found");
            assert_eq!(wallet.balance, user.balance);
            assert_eq!(wallet.first_name, user.first_name);
            assert_eq!(wallet.last_name, user.last_name);
        }
        assert_eq!(schema.wallets.iter().count(), users.len());

        let summary = schema.summary.get().unwrap();
        assert_eq!(summary.ticker, super::SERVICE_NAME);
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

/// Third migration script. Always fails.
fn failing_migration(_ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    Err(MigrationError::Custom("This migration always fails".into()))
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_factory(artifact_name = "exonum.test.Migration", artifact_version = "0.1.0")]
pub struct MigrationService;

impl Service for MigrationService {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        _params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // At the init step fill the schema with some data.
        let mut schema = v01::Schema::new(context.service_data());

        for user in USERS {
            let (key, _) = user.keypair();
            let wallet = v01::Wallet {
                username: user.full_name.to_string(),
                balance: user.balance,
            };
            schema.wallets.put(&key, wallet);
        }

        Ok(())
    }
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_factory(artifact_name = "exonum.test.Migration", artifact_version = "0.2.0")]
pub struct MigrationServiceV02;

impl Service for MigrationServiceV02 {}

/// Service with a fast-forward migration (0.1.0 -> 0.1.1).
#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_factory(artifact_name = "exonum.test.Migration", artifact_version = "0.1.1")]
pub struct MigrationServiceV01_1;

impl Service for MigrationServiceV01_1 {}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_factory(artifact_name = "exonum.test.Migration", artifact_version = "0.5.0")]
pub struct MigrationServiceV05;

impl Service for MigrationServiceV05 {}

/// Service with mixed migrations (data migrations 0.1.0 -> 0.2.0, 0.2.0 -> 0.5.0, and
/// fast-forward migration 0.5.0 -> 0.5.1)
#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_factory(artifact_name = "exonum.test.Migration", artifact_version = "0.5.1")]
pub struct MigrationServiceV05_1;

impl Service for MigrationServiceV05_1 {}

pub const SERVICE_ID: InstanceId = 512;
pub const SERVICE_NAME: &str = "migration-service";

impl DefaultInstance for MigrationService {
    const INSTANCE_ID: InstanceId = SERVICE_ID;
    const INSTANCE_NAME: &'static str = SERVICE_NAME;
}

impl MigrateData for MigrationServiceV02 {
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        LinearMigrations::new(self.artifact_id().version)
            .add_script(Version::new(0, 2, 0), merkelize_wallets)
            .select(start_version)
    }
}

impl MigrateData for MigrationServiceV01_1 {
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        LinearMigrations::new(self.artifact_id().version).select(start_version)
    }
}

impl MigrateData for MigrationServiceV05 {
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

impl MigrateData for MigrationServiceV05_1 {
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

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_factory(artifact_name = "exonum.test.Migration", artifact_version = "0.7.0")]
pub struct FailingMigrationServiceV07;

impl Service for FailingMigrationServiceV07 {}

impl MigrateData for FailingMigrationServiceV07 {
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        LinearMigrations::new(self.artifact_id().version)
            .add_script(Version::new(0, 7, 0), failing_migration)
            .select(start_version)
    }
}
