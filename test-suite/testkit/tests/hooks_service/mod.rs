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

//! A special service which generates transactions on `after_commit` events.

use exonum::runtime::{
    migrations::{InitMigrationError, MigrateData, MigrationScript},
    versioning::Version,
    ExecutionContext, ExecutionError, InstanceId,
};
use exonum_derive::*;
use exonum_rust_runtime::{AfterCommitContext, DefaultInstance, Service};

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

pub const SERVICE_ID: InstanceId = 512;
pub const SERVICE_NAME: &str = "after-commit";

#[exonum_interface(auto_ids)]
pub trait AfterCommitInterface<Ctx> {
    type Output;
    fn after_commit(&self, ctx: Ctx, height: u64) -> Self::Output;
}

#[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
#[service_factory(
    artifact_name = "after-commit",
    artifact_version = "1.0.0",
    service_constructor = "Self::new_instance"
)]
#[service_dispatcher(implements("AfterCommitInterface"))]
pub struct AfterCommitService {
    counter: Arc<AtomicUsize>,
}

impl AfterCommitInterface<ExecutionContext<'_>> for AfterCommitService {
    type Output = Result<(), ExecutionError>;

    fn after_commit(&self, _ctx: ExecutionContext<'_>, _height: u64) -> Self::Output {
        Ok(())
    }
}

impl AfterCommitService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn counter(&self) -> usize {
        self.counter.load(Ordering::SeqCst)
    }

    pub fn switch_to_generic_broadcast(&self) {
        self.counter.store(100_000, Ordering::SeqCst);
    }

    pub fn new_instance(&self) -> Box<dyn Service> {
        Box::new(self.clone())
    }
}

impl Service for AfterCommitService {
    fn after_commit(&self, context: AfterCommitContext<'_>) {
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);

        // Test both validator-specific and generic sending.
        let height = context.height().0;
        if counter < 10_000 {
            if let Some(broadcast) = context.broadcaster() {
                broadcast.blocking().after_commit((), height).ok();
            }
        } else {
            context
                .generic_broadcaster()
                .blocking()
                .after_commit((), height)
                .ok();
        }
    }
}

impl DefaultInstance for AfterCommitService {
    const INSTANCE_ID: u32 = SERVICE_ID;
    const INSTANCE_NAME: &'static str = SERVICE_NAME;
}

#[derive(Debug, Clone, Copy, ServiceFactory, ServiceDispatcher)]
#[service_factory(artifact_name = "after-commit", artifact_version = "2.0.0")]
#[service_dispatcher(implements("AfterCommitInterface"))]
pub struct AfterCommitServiceV2;

impl AfterCommitInterface<ExecutionContext<'_>> for AfterCommitServiceV2 {
    type Output = Result<(), ExecutionError>;

    fn after_commit(&self, _ctx: ExecutionContext<'_>, _height: u64) -> Self::Output {
        Ok(())
    }
}

impl Service for AfterCommitServiceV2 {
    fn after_commit(&self, context: AfterCommitContext<'_>) {
        if let Some(broadcast) = context.broadcaster() {
            let height = context.height().0;
            broadcast.blocking().after_commit((), height).ok();
        }
    }
}

impl MigrateData for AfterCommitServiceV2 {
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        if *start_version == Version::new(1, 0, 0) {
            let script = MigrationScript::new(|_context| Ok(()), Version::new(2, 0, 0));
            Ok(vec![script])
        } else {
            Err(InitMigrationError::NotSupported)
        }
    }
}
