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

// FIXME: Decouple after #1639 is merged.
pub use semver::Version;

use exonum_merkledb::migration::MigrationHelper;

use std::{collections::BTreeMap, fmt};

use crate::runtime::{ArtifactId, InstanceSpec};

/// Atomic migration script.
pub struct MigrationScript {
    name: String,
    logic: Box<dyn FnOnce(&mut MigrationContext) + Send>,
}

impl fmt::Debug for MigrationScript {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MigrationScript")
            .field("name", &self.name)
            .finish()
    }
}

impl MigrationScript {
    /// Creates a new migration script with the specified name and implementation.
    pub fn new<F>(name: impl Into<String>, logic: F) -> Self
    where
        F: FnOnce(&mut MigrationContext) + Send + 'static,
    {
        Self {
            name: name.into(),
            logic: Box::new(logic),
        }
    }

    /// Returns the name of the script.
    pub fn name(&self) -> &str {
        &self.name
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
    pub instance_spec: InstanceSpec,
}

/// Encapsulates data migration logic.
pub trait MigrateData {
    /// Provides a list of data migration scripts to execute in order to arrive from
    /// the `start_version` of the service to the `end_version`. The list may be empty
    /// if no data migrations have occurred between versions. The scripts in the list will
    /// be executed successively in the specified order, flushing the migration to the DB
    /// after each script is completed.
    ///
    /// # Errors
    ///
    /// The error signifies that the service artifact does not know how to migrate
    /// from the start version. This may be the case if the service version is too old,
    /// or too new.
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, DataMigrationError>;
}

/// Errors that can occur during data migrations.
#[derive(Debug)]
pub enum DataMigrationError {
    /// Start version is too far in the past.
    UnsupportedStartVersion {
        /// Minimum supported version.
        min_supported_version: Version,
    },
    /// Start version is in the future.
    FutureStartVersion {
        /// Maximum supported version.
        max_supported_version: Version,
    },
    // TODO: probably need custom errors.
}

/// Linearly ordered migrations.
pub struct LinearMigrations {
    min_start_version: Option<Version>,
    latest_version: Version,
    scripts: BTreeMap<Version, MigrationScript>,
}

impl fmt::Debug for LinearMigrations {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LinearMigrations")
            .field("min_start_version", &self.min_start_version)
            .field("latest_version", &self.latest_version)
            .field("scripts", &self.scripts.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl LinearMigrations {
    /// Creates a new set of migrations with the latest supported version taken
    /// from the supplied artifact.
    pub fn new(latest_artifact: ArtifactId) -> Self {
        Self {
            min_start_version: None,
            latest_version: latest_artifact.version,
            scripts: BTreeMap::new(),
        }
    }

    /// Signals to return an error if the starting version is less than the specified version.
    pub fn forget_before(mut self, version: Version) -> Self {
        self.min_start_version = Some(version);
        self
    }

    /// Adds a migration script at the specified version.
    pub fn migrate<F>(mut self, version: Version, script: F) -> Self
    where
        F: FnOnce(&mut MigrationContext) + Send + 'static,
    {
        let script_name = format!("Migration to version {}", version);
        let script = MigrationScript::new(script_name, script);
        self.scripts.insert(version, script);
        self
    }

    /// Selects a list of migration scripts based on the provided start version of the artifact.
    pub fn select(
        self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, DataMigrationError> {
        if *start_version > self.latest_version {
            return Err(DataMigrationError::FutureStartVersion {
                max_supported_version: self.latest_version,
            });
        }
        if let Some(min_supported_version) = self.min_start_version {
            if *start_version < min_supported_version {
                return Err(DataMigrationError::UnsupportedStartVersion {
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

    // FIXME: tests for linear migrations
}
