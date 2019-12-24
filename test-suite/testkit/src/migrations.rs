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

use exonum::{
    merkledb::{
        access::Prefixed, migration::MigrationHelper, Database, Fork, Snapshot, TemporaryDB,
    },
    runtime::{
        migrations::{MigrateData, MigrationContext, Version},
        rust::ServiceFactory,
        InstanceSpec,
    },
};

use std::sync::Arc;

/// Helper for migration testing.
#[derive(Debug)]
pub struct MigrationTest<S> {
    db: Arc<dyn Database>,
    service_factory: S,
    start_version: Version,
    start_snapshot: Option<Box<dyn Snapshot>>,
    end_snapshot: Option<Box<dyn Snapshot>>,
}

impl<S> MigrationTest<S>
where
    S: ServiceFactory + MigrateData,
{
    const SERVICE_NAME: &'static str = "test";

    /// Initializes a test with the given start version of the artifact.
    pub fn new(service_factory: S, start_version: Version) -> Self {
        Self {
            db: Arc::new(TemporaryDB::new()),
            service_factory,
            start_version,
            start_snapshot: None,
            end_snapshot: None,
        }
    }

    /// Sets up initial data.
    pub fn setup<F>(&mut self, setup: F) -> &mut Self
    where
        F: FnOnce(Prefixed<'static, &Fork>),
    {
        let fork = self.db.fork();
        let access = Prefixed::new(Self::SERVICE_NAME, &fork);
        setup(access);
        self.db.merge(fork.into_patch()).unwrap();
        self
    }

    /// Performs the migration and returns the migrated data.
    pub fn migrate(&mut self) -> &mut Self {
        let mut artifact = self.service_factory.artifact_id();
        artifact.version = self.start_version.clone();
        let instance = InstanceSpec {
            id: 100,
            name: "test".to_owned(),
            artifact,
        };

        let scripts = self
            .service_factory
            .migration_scripts(&self.start_version)
            .expect("Failed to extract migration scripts");

        self.start_snapshot = Some(self.db.snapshot());

        for script in scripts {
            let mut context = MigrationContext {
                helper: MigrationHelper::new(Arc::clone(&self.db), Self::SERVICE_NAME),
                instance_spec: instance.clone(),
            };
            script.execute(&mut context);
            context.helper.finish().unwrap();

            let mut fork = self.db.fork();
            fork.flush_migration(&Self::SERVICE_NAME);
            self.db.merge(fork.into_patch()).unwrap();
        }

        self.end_snapshot = Some(self.db.snapshot());
        self
    }

    /// Gets the snapshot before the migration scripts are run.
    pub fn start_snapshot(&self) -> Prefixed<'static, &dyn Snapshot> {
        let snapshot = self
            .start_snapshot
            .as_ref()
            .expect("Cannot take snapshot before `migrate` method is called");
        Prefixed::new(Self::SERVICE_NAME, snapshot)
    }

    /// Gets the snapshot at the end of the migration.
    pub fn end_snapshot(&self) -> Prefixed<'static, &dyn Snapshot> {
        let snapshot = self
            .end_snapshot
            .as_ref()
            .expect("Cannot take snapshot before `migrate` method is called");
        Prefixed::new(Self::SERVICE_NAME, snapshot)
    }
}
