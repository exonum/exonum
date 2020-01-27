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
    merkledb::{
        access::{Access, FromAccess},
        ProofEntry,
    },
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

#[derive(Debug, FromAccess)]
pub struct Schema<T: Access> {
    pub entry: ProofEntry<T::Base, u8>,
}

impl<T: Access> Schema<T> {
    pub fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

/// Initial service schema.
pub mod v01 {
    use super::Schema;
    use exonum::merkledb::{access::Prefixed, Snapshot};

    pub(super) const ENTRY_VALUE: u8 = 1;

    pub fn verify_schema(snapshot: Prefixed<&dyn Snapshot>) {
        let schema = Schema::new(snapshot.clone());
        assert_eq!(schema.entry.get().expect("No value for entry"), ENTRY_VALUE);
    }
}

pub mod v02 {
    use exonum::merkledb::{access::Prefixed, Snapshot};

    use super::Schema;

    pub(super) const ENTRY_VALUE: u8 = 2;

    pub fn verify_schema(snapshot: Prefixed<&dyn Snapshot>) {
        let schema = Schema::new(snapshot.clone());
        assert_eq!(schema.entry.get().expect("No value for entry"), ENTRY_VALUE);
    }
}

pub mod v05 {
    use exonum::merkledb::{access::Prefixed, Snapshot};

    use super::Schema;

    pub(super) const ENTRY_VALUE: u8 = 5;

    pub fn verify_schema(snapshot: Prefixed<&dyn Snapshot>) {
        let schema = Schema::new(snapshot.clone());
        assert_eq!(schema.entry.get().expect("No value for entry"), ENTRY_VALUE);
    }
}

fn migrate_to_02(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    let mut new_schema = Schema::new(ctx.helper.new_data());
    new_schema.entry.set(v02::ENTRY_VALUE);
    Ok(())
}

fn migrate_to_05(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    let mut new_schema = Schema::new(ctx.helper.new_data());
    new_schema.entry.set(v05::ENTRY_VALUE);
    Ok(())
}

/// Third migration script. Always fails.
fn failing_migration(_ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    Err(MigrationError::new("This migration always fails"))
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
        let mut schema = Schema::new(context.service_data());

        schema.entry.set(v01::ENTRY_VALUE);

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
            .add_script(Version::new(0, 2, 0), migrate_to_02)
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
            .add_script(Version::new(0, 2, 0), migrate_to_02)
            .add_script(Version::new(0, 5, 0), migrate_to_05)
            .select(start_version)
    }
}

impl MigrateData for MigrationServiceV05_1 {
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        LinearMigrations::new(self.artifact_id().version)
            .add_script(Version::new(0, 2, 0), migrate_to_02)
            .add_script(Version::new(0, 5, 0), migrate_to_05)
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
