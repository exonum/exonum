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

//! Data migration tools.
//!
//! # Stability
//!
//! Since migrations are tightly related to unstable [`Runtime`] trait, the entirety of this
//! module is considered unstable.
//!
//! # Migrations Overview
//!
//! The goal of a data migration is to prepare data of an Exonum service for use with an updated
//! version of the service business logic. In this sense, migrations fulfil the same role
//! as migrations in traditional database management systems.
//!
//! Migrations are performed via [`MigrationScript`]s, which are essentially wrappers around
//! a closure. A script takes data of a service and uses the [database capabilities] to transform
//! it to a new version. Migration is non-destructive, i.e., does not remove the old versions
//! of migrated indexes. Instead, new indexes are created in a separate namespace, and atomically
//! replace the old data when the migration is flushed.
//! (See [database docs][database capabilities] for more details.)
//!
//! The problems solved by the migration workflow are:
//!
//! - Allowing for migration to be performed in background, while the node continues to process
//!   transactions and other requests.
//! - Ensuring that migrations finish at finite time (i.e., at some blockchain height).
//! - Allowing concurrent migrations for different services.
//! - Ensuring that all nodes in the network have arrived at the same data after migration
//!   is completed.
//!
//! Similar to other service lifecycle events, data migrations are managed by the [dispatcher],
//! but are controlled by the [supervisor service].
//!
//! # Migration Types
//!
//! Exonum recognizes two kinds of migrations:
//!
//! - **Fast-forward migrations** synchronously change the version
//!   of the artifact associated with the service. A fast-forward migration is performed
//!   if the updated artifact signals that it is compatible with the old service data
//!   by returning `Ok(None)` from [`Runtime::migrate()`].
//! - Migrations that require changing data layout via [`MigrationScript`]s are referred to
//!   as **async migrations**.
//!
//! For a migration to start, the targeted service must be stopped or frozen, and a newer version of
//! the service artifact needs to be deployed across the network.
//!
//! # Fast-Forward Migration Workflow
//!
//! Fast-forward migrations do not require any special workflow to agree migration
//! outcome among nodes; indeed, the outcome is agreed upon via the consensus algorithm.
//! The artifact associated with the service instance is changed instantly.
//! The service status is changed to stopped, regardless of the status before
//! the migration. This is because a new artifact might want to prepare service data
//! before the artifact can use it.
//!
//! # Async Migration Workflow
//!
//! 1. Migration is *initiated* by a call from a supervisor. Once a block with this call is merged,
//!   all nodes in the network retrieve the migration script via [`Runtime::migrate()`]
//!   and start executing it in a background thread. The script may execute at varying speed
//!   on different nodes. Service status changes to [`Migrating`].
//!
//! 2. After the script is finished on a node, its result becomes available using
//!   the [`local_migration_result()`] method of the dispatcher schema. Nodes synchronize
//!   these results using supervisor capabilities (e.g., via broadcasting transactions).
//!
//! 3. Once the consensus is built up around migration, its result is either *committed* or
//!   the migration is *rolled back*. Right below, we consider commitment workflow; the rollback
//!   workflow will be described slightly later.
//!
//! 4. Committing a migration works similarly to [artifact commitment]. It means that any node
//!   in the network starting from a specific blockchain height must have migration completed
//!   with a specific outcome (i.e., hash of the migrated data). A node that does not have
//!   migration script completed by this moment will block until the script is completed.
//!   If the local migration outcome differs from the committed one, the node will be unable
//!   to continue participating in the network.
//!
//! 5. After migration commitment, migration can be *flushed*, which will replace old service data
//!   with the migrated one. Flushing is a separate call to the dispatcher; it can
//!   occur at any block after the migration commitment (since at this point, we guarantee that
//!   the migration data is available and is the same on all nodes).
//!
//! 6. After the migration is flushed, the service returns to the [`Stopped`] status. The service
//!   can then be resumed with the new data, or more migrations could be applied to it.
//!
//! If the migration is rolled back on step 3, the migrated data is erased, and the service
//! returns to the [`Stopped`] status. The local migration result is ignored; if the migration
//! script has not completed locally, it is aborted.
//!
//! Note that commitment and flushing are separate operations and must be performed in
//! different blocks. When a migration is flushed, the migrated data needs to have a definite
//! state, which is ensured by an earlier commitment acting as a filter. The requirement
//! for different blocks is more nuanced and is related to implementation details of the database
//! backend. Namely, the flushing operation needs to be performed on a fork which contains the final
//! migration state; not doing this may break state aggregation.
//!
//! Deciding when it is appropriate to commit or roll back a migration is the responsibility
//! of the supervisor service. For example, it may commit the migration once all validators have
//! submitted identical migration results, and roll back a migration if at least one validator
//! has reported an error during migration or there is divergence among reported migration results.
//!
//! [`Runtime`]: ../trait.Runtime.html
//! [`Runtime::migrate()`]: ../trait.Runtime.html#method.migrate
//! [dispatcher]: ../index.html
//! [supervisor service]: ../index.html#supervisor-service
//! [`MigrationScript`]: struct.MigrationScript.html
//! [database capabilities]: https://docs.rs/exonum-merkledb/latest/exonum_merkledb/migration/
//! [`Runtime::migrate()`]: ../trait.Runtime.html#tymethod.migrate
//! [`Migrating`]: ../enum.InstanceStatus.html#variant.Migrating
//! [`local_migration_result()`]: ../struct.DispatcherSchema.html#method.local_migration_result
//! [artifact commitment]: ../index.html#artifact-lifecycle
//! [`Stopped`]: ../enum.InstanceStatus.html#variant.Stopped

pub use super::types::{InstanceMigration, MigrationStatus};

use exonum_merkledb::migration::{self as db_migration, MigrationHelper};
use semver::Version;
use thiserror::Error;

use std::{collections::BTreeMap, fmt};

use crate::runtime::{CoreError, ExecutionError, ExecutionFail, InstanceSpec};

type MigrationLogic = dyn FnOnce(&mut MigrationContext) -> Result<(), MigrationError> + Send;

/// Types of data migrations.
#[derive(Debug)]
#[non_exhaustive]
pub enum MigrationType {
    /// Fast-forward migration, that is, migration that does not actually change the data layout
    /// of a service.
    FastForward,
    /// Asynchronous data migration that can change the data layout of a service.
    Async,
}

/// Errors that can occur in a migration script.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MigrationError {
    /// Error has occurred in the helper code, due to either a database-level failure (e.g.,
    /// we've run out of disc space) or the migration script getting aborted.
    ///
    /// Scripts should not instantiate errors of this kind.
    #[error("{}", _0)]
    Helper(#[source] db_migration::MigrationError),

    /// Custom error signalling that the migration cannot be completed.
    #[error("{}", _0)]
    Custom(String),
}

impl MigrationError {
    /// Creates a new migration error.
    pub fn new(cause: impl fmt::Display) -> Self {
        Self::Custom(cause.to_string())
    }
}

impl From<db_migration::MigrationError> for MigrationError {
    fn from(err: db_migration::MigrationError) -> Self {
        Self::Helper(err)
    }
}

/// Atomic migration script.
///
/// # Return Value
///
/// A script returns a `Result`. If the script returns [`MigrationError`] constructed
/// by the script, or if the script panics, then the local outcome of a migration will be set
/// to an error. This should be considered a last resort measure; migration logic should
/// prefer to signal inability to perform migration via [`InitMigrationError`] when the migration
/// script is instantiated.
///
/// On the other hand, it is perfectly reasonable to return an error from the script
/// if it is aborted; i.e., to bubble up errors occurring in [`MigrationHelper`].
/// An error in this case *does not* set the local migration result.
///
/// # Design Recommendations
///
/// Migration scripts may be aborted; see [`MigrationHelper`] docs for more technical details.
/// The [migration workflow] aborts the script if the migration is rolled back; also, all running
/// migration scripts are aborted if the node is shut down for whatever reason. Because of this,
/// script writers are encouraged to take aborts into account.
///
/// A good way to handle abortion is to merge changes to the database with sufficient frequency
/// (e.g., approximately once per second), and to let merge errors bubble up.
/// In this case, script abortion will lead to an error during merge, which will terminate
/// the script timely.
///
/// Another reason to merge changes to the database periodically is to avoid out-of-memory errors.
/// Since data changes are stored in RAM before the merge, *not* merging the changes can consume
/// significant memory if the amount of migrated data is large.
///
/// [`MigrationError`]: enum.MigrationError.html
/// [`InitMigrationError`]: enum.InitMigrationError.html
/// [`MigrationHelper`]: https://docs.rs/exonum-merkledb/latest/exonum_merkledb/migration/struct.MigrationHelper.html
/// [migration workflow]: index.html#migration-workflow
pub struct MigrationScript {
    end_version: Version,
    name: String,
    logic: Box<MigrationLogic>,
}

impl fmt::Debug for MigrationScript {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MigrationScript")
            .field("end_version", &self.end_version)
            .field("name", &self.name)
            .finish()
    }
}

impl MigrationScript {
    /// Creates a new migration script with the specified end version and implementation.
    pub fn new<F>(logic: F, end_version: Version) -> Self
    where
        F: FnOnce(&mut MigrationContext) -> Result<(), MigrationError> + Send + 'static,
    {
        Self {
            name: format!("Migration to {}", end_version),
            end_version,
            logic: Box::new(logic),
        }
    }

    /// Returns the name of the script.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the version of the data after this script is applied. This service [`data_version`]
    /// will be set to this value after the migration performed by this script is flushed.
    ///
    /// [`MigrationContext`]: struct.MigrationContext.html
    /// [`data_version`]: ../struct.InstanceState.html#field.data_version
    pub fn end_version(&self) -> &Version {
        &self.end_version
    }

    /// Executes the script.
    pub fn execute(self, context: &mut MigrationContext) -> Result<(), MigrationError> {
        (self.logic)(context)
    }
}

/// Context of a migration.
#[derive(Debug)]
#[non_exhaustive]
pub struct MigrationContext {
    /// The migration helper allowing to access service data and prepare migrated data.
    pub helper: MigrationHelper,

    /// Specification of the migrated instance.
    pub instance_spec: InstanceSpec,

    /// Version of the service data.
    ///
    /// Note that the artifact version will change with each executed [`MigrationScript`]
    /// to reflect the latest version of the service data. For example, if a [`MigrateData`]
    /// implementation produces two scripts, which migrate service data to versions
    /// 0.5.0 and 0.6.0 respectively, then the second script will get the `data_version`
    /// set to 0.5.0, regardless of the original version of the instance artifact.
    ///
    /// [`MigrationScript`]: struct.MigrationScript.html
    /// [`MigrateData`]: trait.MigrateData.html
    pub data_version: Version,
}

impl MigrationContext {
    /// Creates a new `MigrationContext`. Public for `testkit`.
    #[doc(hidden)]
    pub fn new(
        helper: MigrationHelper,
        instance_spec: InstanceSpec,
        data_version: Version,
    ) -> Self {
        Self {
            helper,
            instance_spec,
            data_version,
        }
    }
}

/// Encapsulates data migration logic.
pub trait MigrateData {
    /// Provides a list of data migration scripts to execute in order to arrive from
    /// the `start_version` of the service to the current version. The list may be empty
    /// if no data migrations have occurred between versions. The scripts in the list will
    /// be executed successively in the specified order, flushing the migration to the DB
    /// after each script is completed.
    ///
    /// Inability to migrate data should be eagerly signalled via an [`InitMigrationError`]
    /// returned from this method whenever this is possible. In other words, it should not
    /// be signalled via an error *within* the script.
    ///
    /// # Expectations
    ///
    /// The following constraints are expected from the migration scripts:
    ///
    /// - Scripts should be ordered by increasing [`end_version()`]
    /// - The `end_version` of the initial script should be greater than `start_version`
    /// - The `end_version` of the last script does not need to equal the target
    ///   version of the artifact, but it should not exceed the target version
    ///
    /// # Errors
    ///
    /// The error signifies that the service artifact does not know how to migrate
    /// from the start version. For example, this may be the case if the service version is too old,
    /// or too new.
    ///
    /// [`end_version()`]: struct.MigrationScript.html#method.end_version
    /// [`InitMigrationError`]: enum.InitMigrationError.html
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError>;
}

/// Errors that can occur when initiating a data migration. This error indicates that the migration
/// cannot be started.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum InitMigrationError {
    /// The start version is too far in the past.
    #[error(
        "The provided start version is too far in the past; the minimum supported version is {}",
        min_supported_version
    )]
    OldStartVersion {
        /// Minimum supported version.
        min_supported_version: Version,
    },

    /// The start version is in the future.
    #[error(
        "The provided start version is greater than the maximum supported version ({})",
        max_supported_version
    )]
    FutureStartVersion {
        /// Maximum supported version.
        max_supported_version: Version,
    },

    /// The start version falls in the supported lower / upper bounds on versions,
    /// but is not supported itself. This can be the case, e.g., for pre-releases.
    #[error("Start version is not supported: {}", _0)]
    UnsupportedStart(String),

    /// Data migrations are not supported by the artifact.
    #[error("Data migrations are not supported by the artifact")]
    NotSupported,
}

impl From<InitMigrationError> for ExecutionError {
    fn from(err: InitMigrationError) -> Self {
        CoreError::NoMigration.with_description(err)
    }
}

/// Linearly ordered migrations.
///
/// This type allows to construct a [`MigrateData`] implementation that will follow migrations
/// performed during service evolution. In this way, the mechanism is similar to how migrations
/// are implemented in apps involving relational databases:
///
/// - Migration scripts will be applied to service data in a specific order during evolution
///   of a particular service instance. Each script will be applied exactly once.
/// - Several migration scripts may be applied sequentially if the instance is old enough.
/// - Migrations for different service instances are independent, and migration scripts
///   for them are fully reusable.
///
/// # Retaining Old Data Types
///
/// Migration script logic needs retain the knowledge about data types used in the service
/// in the past. Since these data types may be unused *currently*, retaining them may place
/// a burden on the service. To mitigate this, you can provide a minimum supported starting version
/// of the service via [`set_min_version`](#method.set_min_version).
///
/// # Pre-releases
///
/// Special care must be taken to support pre-release versions (i.e., versions like `0.2.0-pre.2` or
/// `1.2.34-rc.5`). As per the semver specification:
///
/// - Any pre-release version is lesser than a corresponding version without a pre-release suffix
/// - Pre-release versions with the same base triple are ordered alphabetically by their suffixes
///
/// The support of prerelease versions needs to be explicitly enabled by using the
/// [`with_prereleases`](#method.with_prereleases) constructor. Otherwise, a prerelease mentioned
/// in the builder stage will lead to a panic, and [`select`](#method.select) will return an error
/// if a prerelease is specified as a `start_version`.
///
/// [`MigrateData`]: trait.MigrateData.html
///
/// # Examples
///
/// Consider the following hypothetical evolution of a crypto-token service:
///
/// | Version | Migration |
/// |---------|-----------|
/// | 0.2.0   | #1: Split `name` in user accounts into `first_name` and `last_name` |
/// | 0.3.0   | - |
/// | 0.4.0   | #2: Consolidate token metadata into a single `Entry` |
/// | 0.4.1   | - |
/// | 0.4.2   | #3: Compute total number of tokens and add it to metadata |
///
/// In this case:
///
/// - If a service instance is migrated from version 0.1.0 to the newest version 0.4.2, all three
///   scripts need to be executed.
/// - If an instance is migrated from 0.2.0 or 0.3.0, only scripts #2 and #3 need to be executed.
/// - If an instance is migrated from 0.4.0 or 0.4.1, only script #3 needs to be executed.
/// - If the instance version is 0.4.2, no scripts need to be executed.
///
/// The migrations can be described in the service code as follows:
///
/// ```
/// # use exonum::runtime::{
/// #     migrations::{
/// #         LinearMigrations, MigrateData, MigrationContext, MigrationError, MigrationScript,
/// #         InitMigrationError,
/// #     },
/// #     versioning::Version,
/// # };
/// # use exonum_derive::*;
/// fn split_account_name(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
///     // script logic...
/// #   Ok(())
/// }
///
/// fn consolidate_metadata(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
///     // script logic...
/// #   Ok(())
/// }
///
/// fn compute_total_tokens(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
///     // script logic...
/// #   Ok(())
/// }
///
/// fn migrations() -> LinearMigrations {
///     LinearMigrations::new(Version::new(0, 4, 3))
///         .add_script(Version::new(0, 2, 0), split_account_name)
///         .add_script(Version::new(0, 4, 0), consolidate_metadata)
///         .add_script(Version::new(0, 4, 2), compute_total_tokens)
/// }
///
/// /// Service with migrations.
/// pub struct TokenService;
///
/// impl MigrateData for TokenService {
///     fn migration_scripts(
///         &self,
///         start_version: &Version,
///     ) -> Result<Vec<MigrationScript>, InitMigrationError> {
///         migrations().select(start_version)
///     }
/// }
///
/// // Check that the migration scripts are selected properly.
/// # fn main() -> anyhow::Result<()> {
/// let scripts = TokenService.migration_scripts(&Version::new(0, 3, 0))?;
/// assert_eq!(scripts.len(), 2);
/// assert_eq!(*scripts[0].end_version(), Version::new(0, 4, 0));
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct LinearMigrations {
    min_start_version: Option<Version>,
    latest_version: Version,
    scripts: BTreeMap<Version, MigrationScript>,
    support_prereleases: bool,
}

impl LinearMigrations {
    /// Creates a new set of migrations with the specified latest supported version.
    ///
    /// # Panics
    ///
    /// - If `latest_version` is a prerelease.
    pub fn new(latest_version: Version) -> Self {
        assert!(
            !latest_version.is_prerelease(),
            "Prerelease versions require using `with_prereleases` constructor"
        );
        Self {
            support_prereleases: false,
            ..Self::with_prereleases(latest_version)
        }
    }

    /// Creates a new set of migrations with the specified latest supported version.
    ///
    /// Unlike the `new` constructor, this one indicates that prerelease versions are allowed
    /// in the list of migrations and as the start version.
    pub fn with_prereleases(latest_version: Version) -> Self {
        Self {
            min_start_version: None,
            latest_version,
            scripts: BTreeMap::new(),
            support_prereleases: true,
        }
    }

    fn check_prerelease(&self, version: &Version) {
        if !self.support_prereleases {
            assert!(
                !version.is_prerelease(),
                "Prerelease versions require using `with_prereleases` constructor"
            );
        }
    }

    /// Signals to return an error if the starting version is less than the specified version.
    ///
    /// # Panics
    ///
    /// - If `version` is a prerelease and this instance was not created by the `with_prereleases`
    ///   constructor.
    pub fn set_min_version(mut self, version: Version) -> Self {
        self.check_prerelease(&version);
        self.min_start_version = Some(version);
        self
    }

    /// Adds a migration script at the specified version.
    ///
    /// # Panics
    ///
    /// - If `version` is a prerelease and this instance was not created by the `with_prereleases`
    ///   constructor.
    pub fn add_script<F>(mut self, version: Version, script: F) -> Self
    where
        F: FnOnce(&mut MigrationContext) -> Result<(), MigrationError> + Send + 'static,
    {
        self.check_prerelease(&version);
        assert!(
            version <= self.latest_version,
            "Cannot add a script for a future version {} (the latest version is {})",
            version,
            self.latest_version
        );
        let script = MigrationScript::new(script, version.clone());
        self.scripts.insert(version, script);
        self
    }

    fn check(&self, start_version: &Version) -> Result<(), InitMigrationError> {
        if !self.support_prereleases && start_version.is_prerelease() {
            let msg = "the start version is a prerelease".to_owned();
            return Err(InitMigrationError::UnsupportedStart(msg));
        }
        if *start_version > self.latest_version {
            return Err(InitMigrationError::FutureStartVersion {
                max_supported_version: self.latest_version.clone(),
            });
        }
        if let Some(ref min_supported_version) = self.min_start_version {
            if start_version < min_supported_version {
                return Err(InitMigrationError::OldStartVersion {
                    min_supported_version: min_supported_version.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// Selects a list of migration scripts based on the provided start version of the artifact.
    pub fn select(
        self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        self.check(start_version)?;
        Ok(self
            .scripts
            .into_iter()
            .filter_map(|(version, script)| {
                if version > *start_version {
                    Some(script)
                } else {
                    None
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{ArtifactId, RuntimeIdentifier};

    use assert_matches::assert_matches;
    use exonum_crypto::Hash;
    use exonum_merkledb::{
        access::{AccessExt, CopyAccessExt},
        migration::flush_migration,
        Database, Snapshot, TemporaryDB,
    };

    use std::{collections::HashSet, sync::Arc};

    const ARTIFACT_NAME: &str = "service.test.Migration";

    fn migration_02(context: &mut MigrationContext) -> Result<(), MigrationError> {
        assert_eq!(context.instance_spec.name, "test");
        assert_eq!(context.instance_spec.artifact.name, ARTIFACT_NAME);
        assert!(context.data_version < Version::new(0, 2, 0));

        let old_entry = context.helper.old_data().get_proof_entry::<_, u32>("entry");
        assert!(!old_entry.exists());
        let mut new_entry = context.helper.new_data().get_proof_entry::<_, u32>("entry");
        new_entry.set(1);
        Ok(())
    }

    fn migration_05(context: &mut MigrationContext) -> Result<(), MigrationError> {
        assert_eq!(context.instance_spec.name, "test");
        assert_eq!(context.instance_spec.artifact.name, ARTIFACT_NAME);
        assert!(context.data_version >= Version::new(0, 2, 0));
        assert!(context.data_version < Version::new(0, 5, 0));

        let old_entry = context.helper.old_data().get_proof_entry::<_, u32>("entry");
        assert_eq!(old_entry.get(), Some(1));
        let mut new_entry = context.helper.new_data().get_proof_entry::<_, u32>("entry");
        new_entry.set(2);
        Ok(())
    }

    fn migration_06(context: &mut MigrationContext) -> Result<(), MigrationError> {
        assert_eq!(context.instance_spec.name, "test");
        assert_eq!(context.instance_spec.artifact.name, ARTIFACT_NAME);
        assert!(context.data_version >= Version::new(0, 5, 0));
        assert!(context.data_version < Version::new(0, 6, 0));

        let old_entry = context.helper.old_data().get_proof_entry::<_, u32>("entry");
        assert_eq!(old_entry.get(), Some(2));
        let mut new_entry = context.helper.new_data().get_proof_entry::<_, u32>("entry");
        new_entry.set(3);
        Ok(())
    }

    fn create_linear_migrations() -> LinearMigrations {
        LinearMigrations::new(Version::new(0, 6, 3))
            .add_script(Version::new(0, 2, 0), migration_02)
            .add_script(Version::new(0, 5, 0), migration_05)
            .add_script(Version::new(0, 6, 0), migration_06)
    }

    fn execute_scripts(
        db: TemporaryDB,
        start_version: Version,
        scripts: Vec<MigrationScript>,
    ) -> Box<dyn Snapshot> {
        let db = Arc::new(db);
        let artifact = ArtifactId::from_raw_parts(
            RuntimeIdentifier::Rust as _,
            ARTIFACT_NAME.to_owned(),
            start_version.clone(),
        );
        let instance_spec = InstanceSpec::from_raw_parts(100, "test".to_string(), artifact);
        let mut version = start_version;
        let mut migration_hashes = HashSet::new();

        for script in scripts {
            let mut context = MigrationContext::new(
                MigrationHelper::new(Arc::clone(&db) as Arc<dyn Database>, "test"),
                instance_spec.clone(),
                version.clone(),
            );

            let next_version = script.end_version().to_owned();
            assert!(
                version < next_version,
                "current version = {}, next version = {}",
                version,
                next_version
            );
            version = next_version;

            script.execute(&mut context).unwrap();
            let migration_hash = context.helper.finish().unwrap();
            // Since the migration contains `ProofEntry`, its hash should be non-trivial.
            assert_ne!(migration_hash, Hash::zero());
            // Since the value in `ProofEntry` changes with each migration, all `migration_hash`es
            // should be different.
            assert!(migration_hashes.insert(migration_hash));

            let mut fork = db.fork();
            flush_migration(&mut fork, "test");
            db.merge(fork.into_patch()).unwrap();
        }
        db.snapshot()
    }

    #[test]
    fn linear_migration_all_scripts() {
        let migrations = create_linear_migrations();
        assert_eq!(migrations.latest_version, Version::new(0, 6, 3));
        assert_eq!(migrations.min_start_version, None);
        assert_eq!(migrations.scripts.len(), 3);

        let start_version = Version::new(0, 1, 0);
        let scripts = migrations.select(&start_version).unwrap();
        // All 3 scripts should be selected.
        assert_eq!(scripts.len(), 3);
        let snapshot = execute_scripts(TemporaryDB::new(), start_version, scripts);
        let entry = snapshot.get_proof_entry::<_, u32>("test.entry");
        assert_eq!(entry.get(), Some(3));
    }

    #[test]
    fn linear_migration_part_of_scripts() {
        let migrations = create_linear_migrations();
        let start_version = Version::new(0, 2, 0);
        let scripts = migrations.select(&start_version).unwrap();
        // 2 latest scripts should be selected.
        assert_eq!(scripts.len(), 2);

        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_proof_entry::<_, u32>("test.entry").set(1);
        db.merge(fork.into_patch()).unwrap();

        let snapshot = execute_scripts(db, start_version, scripts);
        let entry = snapshot.get_proof_entry::<_, u32>("test.entry");
        assert_eq!(entry.get(), Some(3));
    }

    #[test]
    fn prereleases_are_not_supported_by_default() {
        let migrations = create_linear_migrations();
        let start_version: Version = "0.2.0-pre.2".parse().unwrap();
        let err = migrations.select(&start_version).unwrap_err();
        assert_matches!(err, InitMigrationError::UnsupportedStart(_));
    }

    #[test]
    #[should_panic(expected = "Prerelease versions require using `with_prereleases`")]
    fn prerelease_in_constructor_leads_to_panic() {
        LinearMigrations::new("0.3.0-pre.0".parse().unwrap());
    }

    #[test]
    #[should_panic(expected = "Prerelease versions require using `with_prereleases`")]
    fn prerelease_in_migration_spec_leads_to_panic() {
        LinearMigrations::new(Version::new(0, 3, 1))
            .add_script("0.2.0-pre.1".parse().unwrap(), migration_02);
    }

    #[test]
    fn linear_migration_out_of_bounds_version() {
        let migrations = create_linear_migrations();
        let start_version = Version::new(1, 0, 0);
        let err = migrations.select(&start_version).unwrap_err();
        assert_matches!(
            err,
            InitMigrationError::FutureStartVersion { ref max_supported_version }
                if *max_supported_version == Version::new(0, 6, 3)
        );

        let start_version = Version::new(0, 1, 0);
        let migrations = LinearMigrations::new(Version::new(0, 5, 7))
            .set_min_version(Version::new(0, 4, 0))
            .add_script(Version::new(0, 5, 0), migration_05);
        let err = migrations.select(&start_version).unwrap_err();
        assert_matches!(
            err,
            InitMigrationError::OldStartVersion { ref min_supported_version }
                if *min_supported_version == Version::new(0, 4, 0)
        );
    }

    fn create_migrations_with_prerelease() -> LinearMigrations {
        let pre_version: Version = "0.2.0-alpha.0".parse().unwrap();
        let pre_version_ = pre_version.clone();

        LinearMigrations::with_prereleases(Version::new(0, 3, 2))
            .add_script(pre_version.clone(), move |ctx| {
                let start_version = &ctx.data_version;
                assert!(*start_version < pre_version_);
                ctx.helper.new_data().get_proof_entry("v02pre").set(1_u8);
                Ok(())
            })
            .add_script(Version::new(0, 2, 0), move |ctx| {
                let start_version = &ctx.data_version;
                assert!(*start_version >= pre_version);
                assert!(*start_version < Version::new(0, 2, 0));
                ctx.helper.new_data().get_proof_entry("v02").set(2_u8);
                Ok(())
            })
            .add_script(Version::new(0, 3, 0), |ctx| {
                let start_version = &ctx.data_version;
                assert!(*start_version >= Version::new(0, 2, 0));
                assert!(*start_version < Version::new(0, 3, 0));
                ctx.helper.new_data().get_proof_entry("v03").set(3_u8);
                Ok(())
            })
    }

    #[test]
    fn linear_migration_from_prerelease_with_explicit_allowance() {
        // All scripts should be selected.
        let start_version = Version::new(0, 1, 7);
        let scripts = create_migrations_with_prerelease()
            .select(&start_version)
            .unwrap();
        assert_eq!(scripts.len(), 3);
        execute_scripts(TemporaryDB::new(), start_version, scripts);

        // 2nd and 3rd scripts should be selected (`0.2.0 > 0.2.0-alpha.2 > 0.2.0-alpha.0`).
        let start_version = "0.2.0-alpha.0".parse().unwrap();
        let scripts = create_migrations_with_prerelease()
            .select(&start_version)
            .unwrap();
        assert_eq!(scripts.len(), 2);
        execute_scripts(TemporaryDB::new(), start_version, scripts);

        // ...Still 2nd and 3rd scripts.
        let start_version = "0.2.0-alpha.0".parse().unwrap();
        let scripts = create_migrations_with_prerelease()
            .select(&start_version)
            .unwrap();
        assert_eq!(scripts.len(), 2);
        execute_scripts(TemporaryDB::new(), start_version, scripts);

        // ...Only 3rd script should be selected.
        let start_version = "0.3.0-alpha.0".parse().unwrap();
        let scripts = create_migrations_with_prerelease()
            .select(&start_version)
            .unwrap();
        assert_eq!(scripts.len(), 1);
        execute_scripts(TemporaryDB::new(), start_version, scripts);

        // No scripts at all.
        let start_version = "0.3.0".parse().unwrap();
        let scripts = create_migrations_with_prerelease()
            .select(&start_version)
            .unwrap();
        assert!(scripts.is_empty());
        execute_scripts(TemporaryDB::new(), start_version, scripts);
    }
}
