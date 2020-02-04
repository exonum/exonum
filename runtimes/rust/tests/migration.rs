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
    runtime::{
        migrations::{InitMigrationError, MigrateData, MigrationScript},
        versioning::Version,
        CommonError, CoreError, ErrorMatch, ExecutionContext, ExecutionError, ExecutionFail,
        InstanceStatus, SnapshotExt,
    },
};
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, RustRuntimeBuilder, Service, ServiceFactory};
use pretty_assertions::assert_eq;

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

#[exonum_interface(auto_ids)]
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

impl Withdrawal<ExecutionContext<'_>> for WithdrawalServiceV1 {
    type Output = Result<(), ExecutionError>;

    fn withdraw(&self, context: ExecutionContext<'_>, arg: u64) -> Self::Output {
        let mut schema = Schema::new(context.service_data());
        schema.balance.set(schema.balance.get().unwrap() + arg);
        schema.withdrawals.push(arg);
        Ok(())
    }
}

impl Service for WithdrawalServiceV1 {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
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

/// This implementation fixes the incorrect behavior of the previous one. After the migration
/// procedure, during the resuming this implementation also recalculates the resulting balance.
#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("Withdrawal"))]
#[service_factory(artifact_name = "withdrawal", artifact_version = "0.2.0")]
struct WithdrawalServiceV2;

impl Withdrawal<ExecutionContext<'_>> for WithdrawalServiceV2 {
    type Output = Result<(), ExecutionError>;

    fn withdraw(&self, context: ExecutionContext<'_>, arg: u64) -> Self::Output {
        let mut schema = Schema::new(context.service_data());
        schema.balance.set(schema.balance.get().unwrap() - arg);
        schema.withdrawals.push(arg);
        Ok(())
    }
}

impl Service for WithdrawalServiceV2 {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let mut schema = Schema::new(context.service_data());
        schema.balance.set(u64::from_bytes(params.into()).unwrap());
        Ok(())
    }

    fn resume(&self, context: ExecutionContext<'_>, params: Vec<u8>) -> Result<(), ExecutionError> {
        if !params.is_empty() {
            return Err(CommonError::MalformedArguments
                .with_description("Resuming parameters should be empty."));
        }

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
            .build_for_tests(),
    );
    let events_handle = inspected.events.clone();

    let blockchain = BlockchainBuilder::new(blockchain)
        .with_genesis_config(genesis_config)
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
            RuntimeEvent::StartResumingService(withdrawal_service.clone(), vec![]),
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
            RuntimeEvent::MigrateService(
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
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(4), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::StartResumingService(withdrawal_service.clone(), vec![]),
            RuntimeEvent::AfterTransactions(Height(4), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::CommitService(
                Height(5),
                WithdrawalServiceV2.default_instance().instance_spec,
                InstanceStatus::Active,
            ),
            RuntimeEvent::AfterCommit(Height(5)),
        ]
    );

    // Check instance state after migration and resume.
    let instance_state = blockchain
        .snapshot()
        .for_dispatcher()
        .get_instance(WithdrawalServiceV2::INSTANCE_ID)
        .unwrap();

    assert_eq!(instance_state.spec, withdrawal_service);
    assert_eq!(instance_state.status, Some(InstanceStatus::Active));
    assert_eq!(
        instance_state.data_version(),
        &withdrawal_service.artifact.version
    );

    // Make another withdrawal.
    let amount_2 = 20_000;
    execute_transaction(
        &mut blockchain,
        keypair.withdraw(WithdrawalServiceV2::INSTANCE_ID, amount_2),
    )
    .unwrap();
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(5), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::BeforeTransactions(Height(5), WithdrawalServiceV2::INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(5), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(5), WithdrawalServiceV2::INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(6)),
        ]
    );

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

#[test]
fn test_resume_service_error() {
    let (mut blockchain, _) = create_runtime();
    let keypair = blockchain.as_ref().service_keypair().clone();

    // Make withdrawal.
    let amount = 10_000;
    execute_transaction(
        &mut blockchain,
        keypair.withdraw(WithdrawalServiceV1::INSTANCE_ID, amount),
    )
    .unwrap();

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

    // Resume stopped service instance.
    let actual_err = execute_transaction(
        &mut blockchain,
        keypair.resume_service(
            ToySupervisorService::INSTANCE_ID,
            ResumeService {
                instance_id: WithdrawalServiceV2::INSTANCE_ID,
                artifact: WithdrawalServiceV2.artifact_id(),
                params: vec![1, 2, 3, 4],
            },
        ),
    )
    .unwrap_err();
    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&CommonError::MalformedArguments)
            .with_description_containing("Resuming parameters should be empty")
    );

    // Verify the service instance after migration and unsuccessful resume.
    let instance_state = blockchain
        .snapshot()
        .for_dispatcher()
        .get_instance(WithdrawalServiceV1::INSTANCE_ID)
        .unwrap();

    assert_eq!(
        instance_state.spec,
        WithdrawalServiceV1.default_instance().instance_spec
    );
    assert_eq!(instance_state.status, Some(InstanceStatus::Stopped));
    assert_eq!(
        instance_state.data_version(),
        &WithdrawalServiceV2.artifact_id().version
    );
}

#[test]
fn resume_non_existent_service_error() {
    let (mut blockchain, _) = create_runtime();
    let keypair = blockchain.as_ref().service_keypair().clone();

    let actual_err = execute_transaction(
        &mut blockchain,
        keypair.resume_service(
            ToySupervisorService::INSTANCE_ID,
            ResumeService {
                instance_id: WithdrawalServiceV2::INSTANCE_ID + 1,
                artifact: WithdrawalServiceV2.artifact_id(),
                params: vec![],
            },
        ),
    )
    .unwrap_err();
    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&CoreError::IncorrectInstanceId)
    );
}

#[test]
fn resume_active_service_error() {
    let (mut blockchain, _) = create_runtime();
    let keypair = blockchain.as_ref().service_keypair().clone();

    let actual_err = execute_transaction(
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
    .unwrap_err();
    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&CoreError::ServiceNotStopped)
    );
}
