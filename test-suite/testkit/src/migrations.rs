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

//! Testing framework for data migrations.
//!
//! FIXME: more documentation, examples (ECR-4081)

use exonum::{
    merkledb::{
        access::Prefixed, migration::MigrationHelper, Database, Fork, Snapshot, TemporaryDB,
    },
    runtime::{
        migrations::{MigrateData, MigrationContext, MigrationScript, Version},
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
    data_version: Version,
    start_snapshot: Option<Box<dyn Snapshot>>,
    end_snapshot: Option<Box<dyn Snapshot>>,
}

impl<S> MigrationTest<S>
where
    S: ServiceFactory,
{
    const SERVICE_NAME: &'static str = "test";

    /// Initializes a test with the given start version of the artifact.
    pub fn new(service_factory: S, start_version: Version) -> Self {
        Self {
            db: Arc::new(TemporaryDB::new()),
            service_factory,
            data_version: start_version,
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

    /// Executes a single migration script.
    pub fn execute_script(&mut self, script: MigrationScript) -> &mut Self {
        self.start_snapshot = Some(self.db.snapshot());
        self.do_execute_script(script);
        self.end_snapshot = Some(self.db.snapshot());
        self
    }

    fn do_execute_script(&mut self, script: MigrationScript) {
        let mut artifact = self.service_factory.artifact_id();
        artifact.version = self.data_version.clone();
        let instance_spec = InstanceSpec {
            id: 100,
            name: Self::SERVICE_NAME.to_owned(),
            artifact,
        };

        let mut context = MigrationContext {
            helper: MigrationHelper::new(Arc::clone(&self.db), Self::SERVICE_NAME),
            instance_spec,
        };
        let end_version = script.end_version().to_owned();
        script.execute(&mut context);
        context.helper.finish().unwrap();

        let mut fork = self.db.fork();
        fork.flush_migration(Self::SERVICE_NAME);
        self.db.merge(fork.into_patch()).unwrap();
        self.data_version = end_version;
    }
}

impl<S> MigrationTest<S>
where
    S: ServiceFactory + MigrateData,
{
    /// Performs the migration based on the `MigrateData` implementation.
    pub fn migrate(&mut self) -> &mut Self {
        let scripts = self
            .service_factory
            .migration_scripts(&self.data_version)
            .expect("Failed to extract migration scripts");

        self.start_snapshot = Some(self.db.snapshot());
        for script in scripts {
            self.do_execute_script(script);
        }
        self.end_snapshot = Some(self.db.snapshot());
        self
    }
}

/// Extension trait to build `MigrationScript`s easily.
///
/// # Examples
///
/// ```
/// # use exonum::runtime::migrations::{MigrationContext, MigrationScript};
/// # use exonum_derive::*;
/// use exonum_testkit::migrations::ScriptExt as _;
///
/// fn some_script(ctx: &mut MigrationContext) {
///     // business logic skipped
/// }
///
/// let script: MigrationScript = some_script.with_end_version("0.2.0");
/// ```
pub trait ScriptExt {
    /// Converts a function to a migration script.
    fn with_end_version(self, version: &str) -> MigrationScript;
}

impl<F> ScriptExt for F
where
    F: FnOnce(&mut MigrationContext) + Send + 'static,
{
    fn with_end_version(self, version: &str) -> MigrationScript {
        MigrationScript::new(self, version.parse().expect("Cannot parse end version"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use exonum::runtime::{
        migrations::DataMigrationError,
        rust::{ArtifactProtobufSpec, Service},
        ArtifactId,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn script_1(ctx: &mut MigrationContext) {
        assert_eq!(ctx.instance_spec.artifact.version, Version::new(0, 1, 0));
    }

    fn script_2(ctx: &mut MigrationContext) {
        assert_eq!(ctx.instance_spec.artifact.version, Version::new(0, 2, 0));
    }

    #[derive(Debug, Clone, Default)]
    struct SomeService {
        script_counters: [Arc<AtomicUsize>; 2],
    }

    impl ServiceFactory for SomeService {
        fn artifact_id(&self) -> ArtifactId {
            ArtifactId {
                runtime_id: 0,
                name: "exonum.test.Migrations".to_owned(),
                version: Version::new(0, 3, 2),
            }
        }

        fn artifact_protobuf_spec(&self) -> ArtifactProtobufSpec {
            ArtifactProtobufSpec::default()
        }

        fn create_instance(&self) -> Box<dyn Service> {
            unimplemented!()
        }
    }

    impl MigrateData for SomeService {
        fn migration_scripts(
            &self,
            _: &Version,
        ) -> Result<Vec<MigrationScript>, DataMigrationError> {
            let first_counter = Arc::clone(&self.script_counters[0]);
            let second_counter = Arc::clone(&self.script_counters[1]);

            Ok(vec![
                (move |ctx: &mut MigrationContext| {
                    script_1(ctx);
                    first_counter.fetch_add(1, Ordering::SeqCst);
                })
                .with_end_version("0.2.0"),
                (move |ctx: &mut MigrationContext| {
                    script_2(ctx);
                    second_counter.fetch_add(1, Ordering::SeqCst);
                })
                .with_end_version("0.3.0"),
            ])
        }
    }

    #[test]
    fn start_version_is_updated_between_scripts() {
        let mut test = MigrationTest::new(SomeService::default(), Version::new(0, 1, 0));
        test.setup(|_| {})
            .execute_script(script_1.with_end_version("0.2.0"))
            .execute_script(script_2.with_end_version("0.3.0"));
        assert_eq!(test.data_version, Version::new(0, 3, 0));
    }

    #[test]
    fn migrate_calls_all_scripts() {
        let factory = SomeService::default();
        let mut test = MigrationTest::new(factory.clone(), Version::new(0, 1, 0));
        test.setup(|_| {}).migrate();
        assert_eq!(test.data_version, Version::new(0, 3, 0));
        assert_eq!(factory.script_counters[0].load(Ordering::SeqCst), 1);
        assert_eq!(factory.script_counters[1].load(Ordering::SeqCst), 1);
    }
}
