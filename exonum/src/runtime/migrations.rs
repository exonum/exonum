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

//! Migration tools.
//!
//! FIXME: more details (ECR-4081)

use exonum_merkledb::migration::MigrationHelper;
use failure::Fail;

use std::{collections::BTreeMap, fmt};

use crate::runtime::{versioning::Version, InstanceSpec};

/// Atomic migration script.
pub struct MigrationScript {
    end_version: Version,
    name: String,
    logic: Box<dyn FnOnce(&mut MigrationContext) + Send>,
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
        F: FnOnce(&mut MigrationContext) + Send + 'static,
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

    /// Returns the version of the data after this script is applied. This version will be
    /// as the artifact version in [`MigrationContext`] for the successive migration script
    /// (if any).
    ///
    /// [`MigrationContext`]: struct.MigrationContext.html
    pub fn end_version(&self) -> &Version {
        &self.end_version
    }

    /// Executes the script.
    pub fn execute(self, context: &mut MigrationContext) {
        (self.logic)(context);
    }
}

/// Context of a migration.
#[derive(Debug)]
pub struct MigrationContext {
    /// The migration helper allowing to access service data and prepare migrated data.
    pub helper: MigrationHelper,

    /// Specification of the migrated instance.
    ///
    /// Note that the artifact version will change with each executed [`MigrationScript`]
    /// to reflect the latest version of the service data. For example, if a [`MigrateData`]
    /// implementation produces two scripts, which migrate service data to versions
    /// 0.5.0 and 0.6.0 respectively, then the second script will get the `instance_spec`
    /// with the version set to 0.5.0, regardless of the original version of the instance artifact.
    ///
    /// [`MigrationScript`]: struct.MigrationScript.html
    /// [`MigrateData`]: trait.MigrateData.html
    pub instance_spec: InstanceSpec,
}

/// Encapsulates data migration logic.
pub trait MigrateData {
    /// Provides a list of data migration scripts to execute in order to arrive from
    /// the `start_version` of the service to the current version. The list may be empty
    /// if no data migrations have occurred between versions. The scripts in the list will
    /// be executed successively in the specified order, flushing the migration to the DB
    /// after each script is completed.
    ///
    /// Scripts are not expected to fail; the failure should be eagerly signalled via an error.
    ///
    /// # Expectations
    ///
    /// The following constraints are expected from the migration scripts, although are currently
    /// not checked by the core:
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
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, DataMigrationError>;
}

/// Errors that can occur during data migrations.
#[derive(Debug, Fail)]
pub enum DataMigrationError {
    /// The start version is too far in the past.
    #[fail(
        display = "The provided start version is too far in the past; \
                   the minimum supported version is {}",
        min_supported_version
    )]
    OldStartVersion {
        /// Minimum supported version.
        min_supported_version: Version,
    },

    /// The start version is in the future.
    #[fail(
        display = "The provided start version is greater than the maximum supported version ({})",
        max_supported_version
    )]
    FutureStartVersion {
        /// Maximum supported version.
        max_supported_version: Version,
    },

    /// The start version falls in the supported lower / upper bounds on versions,
    /// but is not supported itself. This can be the case, e.g., for pre-releases.
    #[fail(display = "Start version is not supported: {}", _0)]
    UnsupportedStart(String),

    /// Data migrations are not supported by the artifact.
    #[fail(display = "Data migrations are not supported by the artifact")]
    NotSupported,
}

/// Linearly ordered migrations.
///
/// FIXME: more details, examples (ECR-4081)
///
/// # Limitations
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
        F: FnOnce(&mut MigrationContext) + Send + 'static,
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

    /// Selects a list of migration scripts based on the provided start version of the artifact.
    pub fn select(
        self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, DataMigrationError> {
        if !self.support_prereleases && start_version.is_prerelease() {
            let msg = "the start version is a prerelease".to_owned();
            return Err(DataMigrationError::UnsupportedStart(msg));
        }
        if *start_version > self.latest_version {
            return Err(DataMigrationError::FutureStartVersion {
                max_supported_version: self.latest_version,
            });
        }
        if let Some(min_supported_version) = self.min_start_version {
            if *start_version < min_supported_version {
                return Err(DataMigrationError::OldStartVersion {
                    min_supported_version,
                });
            }
        }

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
    use exonum_merkledb::{access::AccessExt, Database, Snapshot, TemporaryDB};

    use std::{collections::HashSet, sync::Arc};

    const ARTIFACT_NAME: &str = "service.test.Migration";

    fn migration_02(context: &mut MigrationContext) {
        assert_eq!(context.instance_spec.name, "test");
        assert_eq!(context.instance_spec.artifact.name, ARTIFACT_NAME);
        assert!(context.instance_spec.artifact.version < Version::new(0, 2, 0));

        let old_entry = context.helper.old_data().get_proof_entry::<_, u32>("entry");
        assert!(!old_entry.exists());
        let mut new_entry = context.helper.new_data().get_proof_entry::<_, u32>("entry");
        new_entry.set(1);
    }

    fn migration_05(context: &mut MigrationContext) {
        assert_eq!(context.instance_spec.name, "test");
        assert_eq!(context.instance_spec.artifact.name, ARTIFACT_NAME);
        assert!(context.instance_spec.artifact.version >= Version::new(0, 2, 0));
        assert!(context.instance_spec.artifact.version < Version::new(0, 5, 0));

        let old_entry = context.helper.old_data().get_proof_entry::<_, u32>("entry");
        assert_eq!(old_entry.get(), Some(1));
        let mut new_entry = context.helper.new_data().get_proof_entry::<_, u32>("entry");
        new_entry.set(2);
    }

    fn migration_06(context: &mut MigrationContext) {
        assert_eq!(context.instance_spec.name, "test");
        assert_eq!(context.instance_spec.artifact.name, ARTIFACT_NAME);
        assert!(context.instance_spec.artifact.version >= Version::new(0, 5, 0));
        assert!(context.instance_spec.artifact.version < Version::new(0, 6, 0));

        let old_entry = context.helper.old_data().get_proof_entry::<_, u32>("entry");
        assert_eq!(old_entry.get(), Some(2));
        let mut new_entry = context.helper.new_data().get_proof_entry::<_, u32>("entry");
        new_entry.set(3);
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
        let mut version = start_version;
        let mut migration_hashes = HashSet::new();

        for script in scripts {
            let mut context = MigrationContext {
                helper: MigrationHelper::new(Arc::clone(&db) as Arc<dyn Database>, "test"),
                instance_spec: InstanceSpec {
                    id: 100,
                    name: "test".to_string(),
                    artifact: ArtifactId {
                        runtime_id: RuntimeIdentifier::Rust as _,
                        name: ARTIFACT_NAME.to_owned(),
                        version: version.clone(),
                    },
                },
            };

            let next_version = script.end_version().to_owned();
            assert!(
                version < next_version,
                "current version = {}, next version = {}",
                version,
                next_version
            );
            version = next_version;

            script.execute(&mut context);
            let migration_hash = context.helper.finish().unwrap();
            // Since the migration contains `ProofEntry`, its hash should be non-trivial.
            assert_ne!(migration_hash, Hash::zero());
            // Since the value in `ProofEntry` changes with each migration, all `migration_hash`es
            // should be different.
            assert!(migration_hashes.insert(migration_hash));

            let mut fork = db.fork();
            fork.flush_migration("test");
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
        assert_matches!(err, DataMigrationError::UnsupportedStart(_));
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
            DataMigrationError::FutureStartVersion { ref max_supported_version }
                if *max_supported_version == Version::new(0, 6, 3)
        );

        let start_version = Version::new(0, 1, 0);
        let migrations = LinearMigrations::new(Version::new(0, 5, 7))
            .set_min_version(Version::new(0, 4, 0))
            .add_script(Version::new(0, 5, 0), migration_05);
        let err = migrations.select(&start_version).unwrap_err();
        assert_matches!(
            err,
            DataMigrationError::OldStartVersion { ref min_supported_version }
                if *min_supported_version == Version::new(0, 4, 0)
        );
    }

    fn create_migrations_with_prerelease() -> LinearMigrations {
        let pre_version: Version = "0.2.0-alpha.0".parse().unwrap();
        let pre_version_ = pre_version.clone();

        LinearMigrations::with_prereleases(Version::new(0, 3, 2))
            .add_script(pre_version.clone(), move |ctx| {
                let start_version = &ctx.instance_spec.artifact.version;
                assert!(*start_version < pre_version_);
                ctx.helper.new_data().get_proof_entry("v02pre").set(1_u8);
            })
            .add_script(Version::new(0, 2, 0), move |ctx| {
                let start_version = &ctx.instance_spec.artifact.version;
                assert!(*start_version >= pre_version);
                assert!(*start_version < Version::new(0, 2, 0));
                ctx.helper.new_data().get_proof_entry("v02").set(2_u8);
            })
            .add_script(Version::new(0, 3, 0), |ctx| {
                let start_version = &ctx.instance_spec.artifact.version;
                assert!(*start_version >= Version::new(0, 2, 0));
                assert!(*start_version < Version::new(0, 3, 0));
                ctx.helper.new_data().get_proof_entry("v03").set(3_u8);
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
