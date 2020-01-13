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

use exonum::{
    blockchain::{config::InstanceInitParams, Blockchain, BlockchainBuilder, BlockchainMut},
    helpers::Height,
    merkledb::{
        access::{Access, FromAccess},
        BinaryValue, ProofEntry, ProofListIndex,
    },
};
use exonum_derive::*;
use exonum_rust_runtime::{
    migrations::{InitMigrationError, MigrateData, MigrationScript},
    CallContext, CoreError, DefaultInstance, ErrorMatch, ExecutionError, InstanceStatus,
    RustRuntimeBuilder, Service, ServiceFactory, SnapshotExt,
};
use futures::sync::mpsc;
use pretty_assertions::assert_eq;
use semver::Version;

use self::inspected::{
    create_genesis_config_builder, execute_transaction, EventsHandle, Inspected, MigrateService,
    ResumeService, RuntimeEvent, StopService, ToySupervisor, ToySupervisorService,
};

mod inspected;

const INITIAL_BALANCE: u64 = 100_000;

#[derive(Debug, FromAccess, RequireArtifact)]
struct Schema<T: Access> {
    balance: ProofEntry<T::Base, u64>,
    withdrawals: ProofListIndex<T::Base, u64>,
}

impl<T: Access> Schema<T> {
    pub fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

#[exonum_interface]
trait Withdrawal<Ctx> {
    type Output;

    fn withdraw(&self, context: Ctx, arg: u64) -> Self::Output;
}

/// This implementation is incorrect and instead of decrementing the balance of the wallet
/// after withdraw, it increases it.
#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("Withdrawal"))]
#[service_factory(artifact_name = "withdrawal", artifact_version = "0.1.0")]
struct WithdrawalServiceV1;

impl Withdrawal<CallContext<'_>> for WithdrawalServiceV1 {
    type Output = Result<(), ExecutionError>;

    fn withdraw(&self, context: CallContext<'_>, arg: u64) -> Self::Output {
        let mut schema = Schema::new(context.service_data());
        schema.balance.set(schema.balance.get().unwrap() + arg);
        schema.withdrawals.push(arg);
        Ok(())
    }
}

impl Service for WithdrawalServiceV1 {
    fn initialize(&self, context: CallContext<'_>, params: Vec<u8>) -> Result<(), ExecutionError> {
        let mut schema = Schema::new(context.service_data());
        schema.balance.set(u64::from_bytes(params.into()).unwrap());
        Ok(())
    }
}

impl DefaultInstance for WithdrawalServiceV1 {
    const INSTANCE_ID: u32 = 2;
    const INSTANCE_NAME: &'static str = "withdrawal";

    fn default_instance(&self) -> InstanceInitParams {
        self.artifact_id()
            .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            .with_constructor(INITIAL_BALANCE)
    }
}

/// This implementation fixes the incorrect behavior of the previous one. During the migration
/// procedure, the implementation also recalculates the resulting balance.
#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("Withdrawal"))]
#[service_factory(artifact_name = "withdrawal", artifact_version = "0.2.0")]
struct WithdrawalServiceV2;

impl Withdrawal<CallContext<'_>> for WithdrawalServiceV2 {
    type Output = Result<(), ExecutionError>;

    fn withdraw(&self, context: CallContext<'_>, arg: u64) -> Self::Output {
        let mut schema = Schema::new(context.service_data());
        schema.balance.set(schema.balance.get().unwrap() - arg);
        schema.withdrawals.push(arg);
        Ok(())
    }
}

impl Service for WithdrawalServiceV2 {
    fn initialize(&self, context: CallContext<'_>, params: Vec<u8>) -> Result<(), ExecutionError> {
        let mut schema = Schema::new(context.service_data());
        schema.balance.set(u64::from_bytes(params.into()).unwrap());
        Ok(())
    }

    fn resume(&self, context: CallContext<'_>, _params: Vec<u8>) -> Result<(), ExecutionError> {
        // Recalculate the balance taking into account the error of the previous implementation.
        // Despite the simplicity this approach is very fragile and can lead to errors during
        // subsequent migrations.
        // Therefore, this approach should be used only in test environment.
        let mut schema = Schema::new(context.service_data());
        let correct_balance = schema
            .withdrawals
            .iter()
            .fold(schema.balance.get().unwrap(), |balance, value| {
                balance - 2 * value
            });
        schema.balance.set(correct_balance);
        Ok(())
    }
}

impl DefaultInstance for WithdrawalServiceV2 {
    const INSTANCE_ID: u32 = 2;
    const INSTANCE_NAME: &'static str = "withdrawal";

    fn default_instance(&self) -> InstanceInitParams {
        self.artifact_id()
            .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            .with_constructor(INITIAL_BALANCE)
    }
}

impl MigrateData for WithdrawalServiceV2 {
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        assert_eq!(start_version, &WithdrawalServiceV1.artifact_id().version);
        Ok(Vec::new())
    }
}

fn create_runtime() -> (BlockchainMut, EventsHandle) {
    let blockchain = Blockchain::build_for_tests();
    let genesis_config = create_genesis_config_builder()
        .with_artifact(ToySupervisorService.artifact_id())
        .with_instance(ToySupervisorService.default_instance())
        .with_artifact(WithdrawalServiceV1.artifact_id())
        .with_artifact(WithdrawalServiceV2.artifact_id())
        .with_instance(WithdrawalServiceV1.default_instance())
        .build();

    let inspected = Inspected::new(
        RustRuntimeBuilder::new()
            .with_factory(WithdrawalServiceV1)
            .with_migrating_factory(WithdrawalServiceV2)
            .with_factory(ToySupervisorService)
            .build(mpsc::channel(1).0),
    );
    let events_handle = inspected.events.clone();

    let blockchain = BlockchainBuilder::new(blockchain, genesis_config)
        .with_runtime(inspected)
        .build();
    (blockchain, events_handle)
}

#[test]
fn resume_without_migration() {
    let (mut blockchain, events_handle) = create_runtime();
    let keypair = blockchain.as_ref().service_keypair().clone();
    // We are not interested in blockchain initialization events.
    drop(events_handle.take());

    // Make withdrawal.
    let amount = 10_000;
    execute_transaction(
        &mut blockchain,
        keypair.withdraw(WithdrawalServiceV1::INSTANCE_ID, amount),
    )
    .unwrap();
    // We not interested in events in this case.
    drop(events_handle.take());

    // Stop running service instance.
    execute_transaction(
        &mut blockchain,
        keypair.stop_service(
            ToySupervisorService::INSTANCE_ID,
            StopService {
                instance_id: WithdrawalServiceV1::INSTANCE_ID,
            },
        ),
    )
    .unwrap();
    // We not interested in events in this case.
    drop(events_handle.take());

    // Resume stopped service instance.
    execute_transaction(
        &mut blockchain,
        keypair.resume_service(
            ToySupervisorService::INSTANCE_ID,
            ResumeService {
                instance_id: WithdrawalServiceV1::INSTANCE_ID,
                artifact: WithdrawalServiceV1.artifact_id(),
                params: vec![],
            },
        ),
    )
    .unwrap();

    let withdrawal_service = WithdrawalServiceV1.default_instance().instance_spec;
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(3), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::StartResuming(withdrawal_service.clone(), vec![]),
            RuntimeEvent::AfterTransactions(Height(3), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::CommitService(
                Height(4),
                WithdrawalServiceV1.default_instance().instance_spec,
                InstanceStatus::Active,
            ),
            RuntimeEvent::AfterCommit(Height(4)),
        ]
    );

    // Make another withdrawal.
    let amount_2 = 20_000;
    execute_transaction(
        &mut blockchain,
        keypair.withdraw(WithdrawalServiceV1::INSTANCE_ID, amount_2),
    )
    .unwrap();
    drop(events_handle.take());

    // Check balance and history.
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(
        snapshot
            .for_service(WithdrawalServiceV1::INSTANCE_NAME)
            .unwrap(),
    );

    assert_eq!(
        schema.balance.get(),
        Some(INITIAL_BALANCE + amount + amount_2)
    );
    assert_eq!(
        schema.withdrawals.iter().collect::<Vec<_>>(),
        vec![amount, amount_2]
    );
}

#[test]
fn resume_with_fast_forward_migration() {
    let (mut blockchain, events_handle) = create_runtime();
    let keypair = blockchain.as_ref().service_keypair().clone();
    // We are not interested in blockchain initialization events.
    drop(events_handle.take());

    // Make withdrawal.
    let amount = 10_000;
    execute_transaction(
        &mut blockchain,
        keypair.withdraw(WithdrawalServiceV1::INSTANCE_ID, amount),
    )
    .unwrap();
    // We not interested in events in this case.
    drop(events_handle.take());

    // Stop running service instance.
    execute_transaction(
        &mut blockchain,
        keypair.stop_service(
            ToySupervisorService::INSTANCE_ID,
            StopService {
                instance_id: WithdrawalServiceV1::INSTANCE_ID,
            },
        ),
    )
    .unwrap();
    // We not interested in events in this case.
    drop(events_handle.take());

    // Make fast-forward migration to the WithdrawalServiceV2.
    execute_transaction(
        &mut blockchain,
        keypair.migrate_service(
            ToySupervisorService::INSTANCE_ID,
            MigrateService {
                instance_name: WithdrawalServiceV1::INSTANCE_NAME.to_owned(),
                artifact: WithdrawalServiceV2.artifact_id(),
            },
        ),
    )
    .unwrap();

    let withdrawal_service = WithdrawalServiceV2.default_instance().instance_spec;
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(3), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::Migrate(
                withdrawal_service.artifact.clone(),
                WithdrawalServiceV1.artifact_id().version
            ),
            RuntimeEvent::AfterTransactions(Height(3), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(4)),
        ]
    );

    // Resume stopped service instance.
    execute_transaction(
        &mut blockchain,
        keypair.resume_service(
            ToySupervisorService::INSTANCE_ID,
            ResumeService {
                instance_id: WithdrawalServiceV2::INSTANCE_ID,
                artifact: WithdrawalServiceV2.artifact_id(),
                params: vec![],
            },
        ),
    )
    .unwrap();
    drop(events_handle.take());

    // Make another withdrawal.
    let amount_2 = 20_000;
    execute_transaction(
        &mut blockchain,
        keypair.withdraw(WithdrawalServiceV2::INSTANCE_ID, amount_2),
    )
    .unwrap();
    drop(events_handle.take());

    // Check balance and history.
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(
        snapshot
            .for_service(WithdrawalServiceV2::INSTANCE_NAME)
            .unwrap(),
    );

    assert_eq!(
        schema.balance.get(),
        Some(INITIAL_BALANCE - amount - amount_2)
    );
}

#[test]
fn test_resume_incorrect_artifact_version() {
    let (mut blockchain, _) = create_runtime();
    let keypair = blockchain.as_ref().service_keypair().clone();

    execute_transaction(
        &mut blockchain,
        keypair.stop_service(
            ToySupervisorService::INSTANCE_ID,
            StopService {
                instance_id: WithdrawalServiceV1::INSTANCE_ID,
            },
        ),
    )
    .unwrap();

    let actual_err = execute_transaction(
        &mut blockchain,
        keypair.resume_service(
            ToySupervisorService::INSTANCE_ID,
            ResumeService {
                instance_id: WithdrawalServiceV1::INSTANCE_ID,
                artifact: WithdrawalServiceV2.artifact_id(),
                params: vec![],
            },
        ),
    )
    .unwrap_err();
    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&CoreError::CannotResumeService)
    );
}

#[test]
fn test_resume_incorrect_artifact_name() {
    let (mut blockchain, _) = create_runtime();
    let keypair = blockchain.as_ref().service_keypair().clone();

    execute_transaction(
        &mut blockchain,
        keypair.stop_service(
            ToySupervisorService::INSTANCE_ID,
            StopService {
                instance_id: WithdrawalServiceV1::INSTANCE_ID,
            },
        ),
    )
    .unwrap();

    let mut artifact = WithdrawalServiceV1.artifact_id();
    artifact.name = "toy_supervisor".to_owned();

    let actual_err = execute_transaction(
        &mut blockchain,
        keypair.resume_service(
            ToySupervisorService::INSTANCE_ID,
            ResumeService {
                instance_id: WithdrawalServiceV1::INSTANCE_ID,
                artifact,
                params: vec![],
            },
        ),
    )
    .unwrap_err();
    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&CoreError::CannotResumeService)
    );
}
