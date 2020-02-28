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

//! Supervisor surrogate.

use exonum::{
    merkledb::{
        access::{Access, FromAccess},
        HashTag, MapIndex,
    },
    runtime::{
        migrations::MigrationType, ArtifactId, CoreError, ExecutionContext, ExecutionError,
        InstanceId, InstanceStatus, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, Service};
use serde_derive::*;

#[derive(FromAccess)]
struct Schema<T: Access> {
    // Countdowns until migration is considered committed.
    migrations: MapIndex<T::Base, str, u32>,
}

impl<T: Access> Schema<T> {
    fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize, BinaryValue)]
#[binary_value(codec = "bincode")]
pub struct StartMigration {
    pub instance_id: InstanceId,
    pub new_artifact: ArtifactId,
    pub migration_len: u32,
}

#[exonum_interface(auto_ids)]
pub trait SupervisorInterface<Ctx> {
    type Output;

    fn stop_service(&self, context: Ctx, instance_id: InstanceId) -> Self::Output;
    fn freeze_service(&self, context: Ctx, instance_id: InstanceId) -> Self::Output;
    /// Resumes the service with the empty arguments.
    fn resume_service(&self, context: Ctx, instance_id: InstanceId) -> Self::Output;
    fn start_migration(&self, context: Ctx, arg: StartMigration) -> Self::Output;
    fn unload_artifact(&self, context: Ctx, artifact: ArtifactId) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("SupervisorInterface"))]
#[service_factory(artifact_name = "supervisor", artifact_version = "1.0.0")]
pub struct Supervisor;

impl Service for Supervisor {
    // We place this code before transactions in order to not run it in the same block when
    // the migration is initiated.
    fn before_transactions(&self, mut context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        let current_migrations: Vec<_> = Schema::new(context.service_data())
            .migrations
            .iter()
            .collect();

        for (instance_name, countdown) in &current_migrations {
            let instance_status = context
                .data()
                .for_dispatcher()
                .get_instance(instance_name.as_str())
                .expect("Cannot find service info")
                .status
                .expect("Service status cannot change to `None`");
            let is_completed = match instance_status {
                InstanceStatus::Migrating(migration) => migration.is_completed(),
                other => panic!("Unexpected service status: {}", other),
            };

            let mut extensions = context.supervisor_extensions();
            if is_completed {
                extensions.flush_migration(instance_name)?;
                Schema::new(context.service_data())
                    .migrations
                    .remove(instance_name);
            } else if *countdown == 0 {
                extensions.commit_migration(instance_name, HashTag::empty_map_hash())?;
            } else {
                Schema::new(context.service_data())
                    .migrations
                    .put(instance_name, *countdown - 1);
            }
        }
        Ok(())
    }
}

impl DefaultInstance for Supervisor {
    const INSTANCE_ID: u32 = SUPERVISOR_INSTANCE_ID;
    const INSTANCE_NAME: &'static str = "supervisor";
}

impl SupervisorInterface<ExecutionContext<'_>> for Supervisor {
    type Output = Result<(), ExecutionError>;

    fn stop_service(
        &self,
        mut context: ExecutionContext<'_>,
        instance_id: InstanceId,
    ) -> Self::Output {
        context
            .supervisor_extensions()
            .initiate_stopping_service(instance_id)
    }

    fn freeze_service(
        &self,
        mut context: ExecutionContext<'_>,
        instance_id: InstanceId,
    ) -> Self::Output {
        context
            .supervisor_extensions()
            .initiate_freezing_service(instance_id)
    }

    fn resume_service(
        &self,
        mut context: ExecutionContext<'_>,
        instance_id: InstanceId,
    ) -> Self::Output {
        context
            .supervisor_extensions()
            .initiate_resuming_service(instance_id, ())
    }

    fn start_migration(
        &self,
        mut context: ExecutionContext<'_>,
        arg: StartMigration,
    ) -> Self::Output {
        let instance_name = context
            .data()
            .for_dispatcher()
            .get_instance(arg.instance_id)
            .ok_or(CoreError::IncorrectInstanceId)?
            .spec
            .name;

        let migration_type = context
            .supervisor_extensions()
            .initiate_migration(arg.new_artifact.clone(), &instance_name)?;
        if let MigrationType::FastForward = migration_type {
            assert_eq!(arg.migration_len, 0);
            context
                .supervisor_extensions()
                .initiate_resuming_service(arg.instance_id, ())
        } else {
            Schema::new(context.service_data())
                .migrations
                .put(&instance_name, arg.migration_len);
            Ok(())
        }
    }

    fn unload_artifact(
        &self,
        mut context: ExecutionContext<'_>,
        artifact: ArtifactId,
    ) -> Self::Output {
        context.supervisor_extensions().unload_artifact(&artifact)
    }
}
