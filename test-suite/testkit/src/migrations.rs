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

//! Testing framework for data migrations.
//!
//! This module allows to test data migration scripts in isolation, without involving actual
//! migration workflow for service instances or `TestKit` in general.
//!
//! The core type in this module is [`MigrationTest`]; see its docs for examples of usage.
//!
//! # Stability
//!
//! Since the corresponding functionality in the core is unstable, the entirety of this module
//! is considered unstable as well.
//!
//! [`MigrationTest`]: struct.MigrationTest.html

use exonum::{
    merkledb::{
        access::Prefixed,
        migration::{
            flush_migration, AbortMigration, Migration, MigrationError as DbMigrationError,
            MigrationHelper,
        },
        Database, Fork, Snapshot, TemporaryDB,
    },
    runtime::{
        migrations::{MigrateData, MigrationContext, MigrationError, MigrationScript},
        versioning::Version,
        InstanceSpec,
    },
};
use exonum_rust_runtime::ServiceFactory;

use std::{
    iter,
    sync::{Arc, Mutex},
};

/// Status of a migration script execution.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptStatus {
    /// The script has successfully completed and the migration data was flushed.
    Ok,
    /// The script was aborted as per abort policy used.
    Aborted,
}

/// Helper for migration testing.
///
/// The helper implements the following workflow:
///
/// 1. Prepare test data to be migrated using [`setup`](#method.setup).
/// 2. Execute one or more migration scripts using [`execute_script`](#method.execute_script)
///   or [`migrate`](#method.migrate).
/// 3. Check that the migrated data is valid using the [`end_snapshot`](#method.end_snapshot)
///   of the database and, possible, the [snapshot before migration](#method.start_snapshot).
///
/// # Examples
///
/// Testing a single migration script:
///
/// ```
/// # use exonum_derive::*;
/// # use exonum::runtime::{
/// #     migrations::{MigrationContext, MigrationError}, versioning::Version,
/// # };
/// use exonum::merkledb::access::AccessExt;
/// # use exonum_rust_runtime::Service;
/// use exonum_testkit::migrations::{MigrationTest, ScriptExt};
///
/// fn script_under_test(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
///     let old_data = ctx.helper.old_data();
///     let old_value = old_data.get_entry::<_, u32>("entry").get().unwrap_or(0);
///     let new_data = ctx.helper.new_data();
///     new_data.get_proof_entry("entry").set(old_value + 1);
///     Ok(())
/// }
///
/// /// Service under test.
/// #[derive(Debug, ServiceDispatcher, ServiceFactory)]
/// #[service_factory(artifact_name = "test-service")]
/// pub struct ServiceUnderTest;
///
/// impl Service for ServiceUnderTest {}
///
/// let mut test = MigrationTest::new(ServiceUnderTest, Version::new(0, 1, 0));
/// let snapshot = test
///     .setup(|access| {
///         // Setup data for the test, for example, create the old service schema
///         // and add test data into it.
///         access.get_entry("entry").set(1_u32);
///     })
///     .execute_script(script_under_test.with_end_version("0.2.0"))
///     .end_snapshot();
/// // Check the data in the snapshot after the script is executed.
/// let value = snapshot.get_proof_entry::<_, u32>("entry").get();
/// assert_eq!(value, Some(2));
/// ```
///
/// Testing fault tolerance of a script:
///
/// ```
/// # use exonum_derive::*;
/// # use exonum::runtime::{
/// #     migrations::{MigrationContext, MigrationError}, versioning::Version,
/// # };
/// use exonum::merkledb::access::AccessExt;
/// # use exonum_rust_runtime::Service;
/// use exonum_testkit::migrations::{AbortPolicy, MigrationTest, ScriptExt};
///
/// fn script_with_merges(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
///     let new_data = ctx.helper.new_data();
///     let mut counter = new_data.get_entry::<_, u32>("counter").get().unwrap_or(0);
///     while counter < 5 {
///         counter += 1;
///         ctx.helper.new_data().get_entry("counter").set(counter);
///         ctx.helper.merge()?;
///     }
///     Ok(())
/// }
/// # #[derive(Debug, ServiceDispatcher, ServiceFactory)]
/// # #[service_factory(artifact_name = "test-service")]
/// # pub struct ServiceUnderTest;
/// # impl Service for ServiceUnderTest {}
///
/// let mut test = MigrationTest::new(ServiceUnderTest, Version::new(0, 1, 0));
/// let end_snapshot = test
///     .execute_until_flush(
///         || script_with_merges.with_end_version("0.2.0"),
///         AbortPolicy::abort_repeatedly(),
///     )
///     .end_snapshot();
/// // The counter value should be set to 5.
/// let counter = end_snapshot.get_entry::<_, u32>("counter").get();
/// assert_eq!(counter, Some(5));
/// ```
///
/// Testing that a script makes progress (this one doesn't):
///
/// ```should_panic
/// # use exonum_derive::*;
/// # use exonum::runtime::{
/// #     migrations::{MigrationContext, MigrationError}, versioning::Version,
/// # };
/// # use exonum::merkledb::access::AccessExt;
/// # use exonum_rust_runtime::Service;
/// # use exonum_testkit::migrations::{AbortPolicy, MigrationTest, ScriptExt};
/// fn infinite_script(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
///     for counter in 0_u32..5 {
///         ctx.helper.new_data().get_entry("counter").set(counter);
///         ctx.helper.merge()?;
///     }
///     // To get here, the script requires 5 successive database merges to succeed.
///     Ok(())
/// }
/// # #[derive(Debug, ServiceDispatcher, ServiceFactory)]
/// # #[service_factory(artifact_name = "test-service")]
/// # pub struct ServiceUnderTest;
/// # impl Service for ServiceUnderTest {}
///
/// let mut test = MigrationTest::new(ServiceUnderTest, Version::new(0, 1, 0));
/// test.execute_until_flush(
///     || infinite_script.with_end_version("0.2.0"),
///     // This policy does not generate 5 successful merges in a row, so the script
///     // doesn't make any progress. Due to `limit_merges`, the test will panic
///     // rather than hang up.
///     AbortPolicy::abort_repeatedly().limit_merges(100),
/// );
/// ```
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

    /// Sets up initial data for the service before the migration.
    pub fn setup<F>(&mut self, setup: F) -> &mut Self
    where
        F: FnOnce(Prefixed<&Fork>),
    {
        let fork = self.db.fork();
        let access = Prefixed::new(Self::SERVICE_NAME, &fork);
        setup(access);
        self.db.merge(fork.into_patch()).unwrap();
        self
    }

    /// Gets the snapshot before the migration scripts are run.
    pub fn start_snapshot(&self) -> Prefixed<&dyn Snapshot> {
        let snapshot = self
            .start_snapshot
            .as_ref()
            .expect("Cannot take snapshot before `migrate` method is called");
        Prefixed::new(Self::SERVICE_NAME, snapshot)
    }

    /// Gets the migrated data. This method is useful to inspect migration state after
    /// script abortion. Once the migration is flushed, the migrated data is erased.
    pub fn migration_data(&self) -> Migration<&dyn Snapshot> {
        let snapshot = self
            .end_snapshot
            .as_ref()
            .expect("Cannot take snapshot before `migrate` method is called");
        Migration::new(Self::SERVICE_NAME, snapshot)
    }

    /// Gets the snapshot at the end of the migration. If the latest migration script execution
    /// was aborted, this method will provide access to old data since the migration is not
    /// flushed in this case.
    pub fn end_snapshot(&self) -> Prefixed<&dyn Snapshot> {
        let snapshot = self
            .end_snapshot
            .as_ref()
            .expect("Cannot take snapshot before `migrate` method is called");
        Prefixed::new(Self::SERVICE_NAME, snapshot)
    }

    /// Executes a single migration script.
    pub fn execute_script(&mut self, script: MigrationScript) -> &mut Self {
        self.start_snapshot = Some(self.db.snapshot());
        self.do_execute_script(script, ());
        self.end_snapshot = Some(self.db.snapshot());
        self
    }

    /// Executes a migration script with the specified abort policy. Each time the script merges
    /// changes to the database, the policy will be queried whether to proceed or emulate script
    /// abort.
    ///
    /// # Return Value
    ///
    /// Returns status of the script indicating whether it completed successfully or was aborted.
    pub fn execute_script_with_aborts(
        &mut self,
        script: MigrationScript,
        abort_handle: impl AbortMigration + 'static,
    ) -> ScriptStatus {
        self.start_snapshot = Some(self.db.snapshot());
        let status = self.do_execute_script(script, abort_handle);
        self.end_snapshot = Some(self.db.snapshot());
        status
    }

    /// Repeatedly executes a migration script with the provided abort policy until the script
    /// is completed (i.e., returns `Ok(_)`). The migration is flushed after this method returns.
    ///
    /// The policy should be configured in such a way that it eventually progresses
    /// the script. A policy that aborts the script every time will lead to a hang-up
    /// if the script has at least one merge. [`AbortPolicy`] automatically
    /// satisfies this requirement; after each abort, it allows at least one merge.
    ///
    /// [`AbortPolicy`]: struct.AbortPolicy.html
    pub fn execute_until_flush<F, T>(&mut self, mut script_fn: F, aborts: T) -> &mut Self
    where
        F: FnMut() -> MigrationScript,
        T: AbortMigration + Send + Sync + 'static,
    {
        #[derive(Debug)]
        struct Shared<U>(Arc<U>);

        impl<U> Clone for Shared<U> {
            fn clone(&self) -> Self {
                Shared(Arc::clone(&self.0))
            }
        }

        impl<U> AbortMigration for Shared<U>
        where
            U: AbortMigration + Send + Sync,
        {
            fn is_aborted(&self) -> bool {
                self.0.is_aborted()
            }
        }

        let abort_handle = Shared(Arc::new(aborts));
        loop {
            let script = script_fn();
            let res = self.execute_script_with_aborts(script, abort_handle.clone());
            if res == ScriptStatus::Ok {
                break;
            }
        }
        self
    }

    fn do_execute_script(
        &mut self,
        script: MigrationScript,
        abort_handle: impl AbortMigration + 'static,
    ) -> ScriptStatus {
        let instance_spec = InstanceSpec::from_raw_parts(
            100,
            Self::SERVICE_NAME.to_owned(),
            self.service_factory.artifact_id(),
        );

        let mut context = MigrationContext::new(
            MigrationHelper::new(Arc::clone(&self.db), Self::SERVICE_NAME),
            instance_spec,
            self.data_version.clone(),
        );
        context.helper.set_abort_handle(abort_handle);
        let end_version = script.end_version().to_owned();

        match script.execute(&mut context) {
            Ok(()) => {
                // The fork in `MigrationHelper` may contain unmerged changes. We want to merge them,
                // but we need to swap the abort handle first (since the old one may lead to `finish`
                // failing).
                context.helper.set_abort_handle(());
                context.helper.finish().unwrap();

                let mut fork = self.db.fork();
                flush_migration(&mut fork, Self::SERVICE_NAME);
                self.db.merge(fork.into_patch()).unwrap();
                self.data_version = end_version;
                ScriptStatus::Ok
            }
            Err(MigrationError::Helper(DbMigrationError::Aborted)) => {
                // We've successfully emulated script abortion!
                ScriptStatus::Aborted
            }
            Err(err) => panic!("Script has generated a fatal error: {}", err),
        }
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
            self.do_execute_script(script, ());
        }
        self.end_snapshot = Some(self.db.snapshot());
        self
    }
}

/// Abort policy based on an iterator yielding responses to the question whether to abort
/// the migration script when it merges data to the database.
///
/// After each "yes" answer (i.e., script abort) the policy will return "no" to the following
/// question without querying the iterator. This ensures that using `AbortPolicy` in the
/// [`execute_until_flush`] method will eventually complete the script (unless the script logic
/// itself is faulty).
///
/// [`execute_until_flush`]: struct.MigrationTest.html#method.execute_until_flush
#[derive(Debug)]
pub struct AbortPolicy<I> {
    // `Mutex` is used to make the type `Send` / `Sync`.
    inner: Mutex<AbortPolicyInner<I>>,
}

impl<I> AbortPolicy<I>
where
    I: Iterator<Item = bool>,
{
    /// Creates a new policy around the provided iterator. The iterator may be finite; once
    /// it runs out of items, the answer to the question whether to abort the script will always
    /// be "no".
    pub fn new(iter: I) -> Self {
        let inner = AbortPolicyInner {
            iter,
            was_aborted_on_prev_iteration: false,
            merge_count: 0,
            max_merges: None,
        };

        Self {
            inner: Mutex::new(inner),
        }
    }

    /// Limits the maximum number of merges that the policy will execute. If this number is exceeded,
    /// the `AbortPolicy` will panic.
    ///
    /// Limiting the number of merges is useful to test that a migration script does not hang up
    /// with an "unfriendly" abort schedule.
    pub fn limit_merges(mut self, max_merges: usize) -> Self {
        let lock = self
            .inner
            .get_mut()
            .expect("Cannot lock `AbortPolicy` mutex");
        lock.max_merges = Some(max_merges);
        self
    }
}

impl AbortPolicy<iter::Repeat<bool>> {
    /// Creates a policy which aborts the script every other time. This ensures that the script
    /// makes progress, but tests its failure tolerance in "unfriendly" conditions.
    pub fn abort_repeatedly() -> Self {
        Self::new(iter::repeat(true))
    }
}

impl<I> AbortMigration for AbortPolicy<I>
where
    I: Iterator<Item = bool> + Send,
{
    fn is_aborted(&self) -> bool {
        let mut lock = self.inner.lock().expect("Cannot lock `AbortPolicy` mutex");
        lock.should_abort()
    }
}

#[derive(Debug)]
struct AbortPolicyInner<I> {
    iter: I,
    was_aborted_on_prev_iteration: bool,
    merge_count: usize,
    max_merges: Option<usize>,
}

impl<I: Iterator<Item = bool>> AbortPolicyInner<I> {
    fn should_abort(&mut self) -> bool {
        let should_abort = if self.was_aborted_on_prev_iteration {
            self.was_aborted_on_prev_iteration = false;
            false
        } else {
            let next_value = self.iter.next().unwrap_or(false);
            self.was_aborted_on_prev_iteration = next_value;
            next_value
        };

        if !should_abort {
            self.merge_count += 1;
            if let Some(max_merges) = self.max_merges {
                assert!(
                    self.merge_count <= max_merges,
                    "Migration script has not terminated after {} merges",
                    max_merges
                );
            }
        }
        should_abort
    }
}

/// Extension trait to build `MigrationScript`s easily.
///
/// # Examples
///
/// ```
/// # use exonum::runtime::migrations::{MigrationContext, MigrationError, MigrationScript};
/// # use exonum_derive::*;
/// use exonum_testkit::migrations::ScriptExt as _;
///
/// fn some_script(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
///     // business logic skipped
/// #   Ok(())
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
    F: FnOnce(&mut MigrationContext) -> Result<(), MigrationError> + Send + 'static,
{
    fn with_end_version(self, version: &str) -> MigrationScript {
        MigrationScript::new(self, version.parse().expect("Cannot parse end version"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use exonum::{
        merkledb::access::AccessExt,
        runtime::{migrations::InitMigrationError, ArtifactId},
    };
    use exonum_rust_runtime::{ArtifactProtobufSpec, Service};

    use std::{
        iter,
        sync::atomic::{AtomicUsize, Ordering},
    };

    fn script_1(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
        assert_eq!(ctx.data_version, Version::new(0, 1, 0));
        Ok(())
    }

    fn script_2(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
        assert_eq!(ctx.data_version, Version::new(0, 2, 0));
        Ok(())
    }

    /// This script counts to 10.
    fn script_with_merges(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
        let mut current_value = ctx
            .helper
            .new_data()
            .get_entry::<_, u32>("entry")
            .get()
            .unwrap_or(0);

        while current_value < 10 {
            ctx.helper
                .new_data()
                .get_entry::<_, u32>("entry")
                .set(current_value + 1);
            ctx.helper.merge()?;
            current_value += 1;
        }
        Ok(())
    }

    /// This script attempts to count to 10, but fails miserably.
    fn incorrect_script_with_merges(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
        for i in 0_u32..10 {
            ctx.helper.new_data().get_entry("entry").set(i);
            ctx.helper.merge()?;
        }
        Ok(())
    }

    #[derive(Debug, Clone, Default)]
    struct SomeService {
        script_counters: [Arc<AtomicUsize>; 2],
    }

    impl ServiceFactory for SomeService {
        fn artifact_id(&self) -> ArtifactId {
            ArtifactId::from_raw_parts(0, "exonum.test.Migrations".into(), Version::new(0, 3, 2))
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
        ) -> Result<Vec<MigrationScript>, InitMigrationError> {
            let first_counter = Arc::clone(&self.script_counters[0]);
            let second_counter = Arc::clone(&self.script_counters[1]);

            Ok(vec![
                (move |ctx: &mut MigrationContext| {
                    script_1(ctx)?;
                    first_counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .with_end_version("0.2.0"),
                (move |ctx: &mut MigrationContext| {
                    script_2(ctx)?;
                    second_counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
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

    #[test]
    fn abort_policy_with_chosen_iterators() {
        let abort_policy = AbortPolicy::new(iter::repeat(false));
        for _ in 0..100 {
            assert!(!abort_policy.is_aborted());
        }

        let abort_policy = AbortPolicy::new(iter::repeat(true));
        for i in 0..100 {
            // We should get `true, false, true, false, ...` sequence of aborts.
            assert_eq!(abort_policy.is_aborted(), i % 2 == 0);
        }

        let seq = iter::repeat(false).take(3).chain(iter::repeat(true));
        let abort_policy = AbortPolicy::new(seq);
        let expected_aborts = vec![false, false, false, true, false, true, false, true];
        for expected in expected_aborts {
            assert_eq!(expected, abort_policy.is_aborted());
        }

        let seq = vec![false, true, false, false, true];
        let abort_policy = AbortPolicy::new(seq.into_iter());
        let expected_aborts = vec![false, true, false, false, false, true, false, false];
        for expected in expected_aborts {
            assert_eq!(expected, abort_policy.is_aborted());
        }
    }

    #[test]
    fn testing_script_with_aborts() {
        let mut test = MigrationTest::new(SomeService::default(), Version::new(0, 1, 0));
        let abort_policy = AbortPolicy::new(iter::once(true));
        let status = test
            .setup(|_| {})
            .execute_script_with_aborts(script_with_merges.with_end_version("0.2.0"), abort_policy);
        assert_eq!(status, ScriptStatus::Aborted);
        let value = test.end_snapshot().get_entry::<_, u32>("entry").get();
        assert_eq!(value, None);

        let abort_policy = AbortPolicy::new(vec![false, false, true].into_iter());
        let status = test
            .execute_script_with_aborts(script_with_merges.with_end_version("0.2.0"), abort_policy);
        assert_eq!(status, ScriptStatus::Aborted);
        let value = test.migration_data().get_entry::<_, u32>("entry").get();
        assert_eq!(value, Some(2)); // Two successful iterations.

        let status =
            test.execute_script_with_aborts(script_with_merges.with_end_version("0.2.0"), ());
        assert_eq!(status, ScriptStatus::Ok);
        let value = test.end_snapshot().get_entry::<_, u32>("entry").get();
        assert_eq!(value, Some(10));
        assert!(test.migration_data().index_type("entry").is_none());
    }

    #[test]
    fn running_script_until_flush() {
        let abort_policy = AbortPolicy::abort_repeatedly();
        let mut test = MigrationTest::new(SomeService::default(), Version::new(0, 1, 0));
        let fail_counter = Arc::new(AtomicUsize::new(0));

        let fail_counter_ = Arc::clone(&fail_counter);
        let script = |ctx: &mut MigrationContext| {
            script_with_merges(ctx).map_err(move |err| {
                fail_counter_.fetch_add(1, Ordering::SeqCst);
                err
            })
        };

        let snapshot = test
            .execute_until_flush(|| script.clone().with_end_version("0.2.0"), abort_policy)
            .end_snapshot();

        let value = snapshot.get_entry::<_, u32>("entry").get();
        assert_eq!(value, Some(10));
        assert_eq!(fail_counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    #[should_panic(expected = "Migration script has not terminated after 100 merges")]
    fn migration_script_hanging_up() {
        let mut test = MigrationTest::new(SomeService::default(), Version::new(0, 1, 0));
        test.execute_until_flush(
            || incorrect_script_with_merges.with_end_version("0.2.0"),
            AbortPolicy::abort_repeatedly().limit_merges(100),
        );
    }
}
