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
    blockchain::{
        config::{GenesisConfig, GenesisConfigBuilder, InstanceInitParams},
        Blockchain, BlockchainBuilder, BlockchainMut,
    },
    helpers::{generate_testnet_config, Height},
    merkledb::{
        access::{Access, FromAccess, RawAccessMut},
        BinaryValue, ProofEntry, ProofListIndex,
    },
};
use exonum_derive::*;
use exonum_rust_runtime::{
    CallContext, CoreError, DefaultInstance, ExecutionError, InstanceStatus, RustRuntimeBuilder,
    Service, ServiceFactory, SnapshotExt,
};
use futures::sync::mpsc;
use pretty_assertions::assert_eq;

use self::inspected::{
    create_block_with_transactions, create_genesis_config_builder, execute_transaction,
    DeployArtifact, EventsHandle, Inspected, ResumeService, RuntimeEvent, StartService,
    StopService, ToySupervisor, ToySupervisorService,
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

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("Withdrawal"))]
#[service_factory(artifact_name = "withdrawal", artifact_version = "0.1.0")]
struct WithdrawalServiceV1;

impl Withdrawal<CallContext<'_>> for WithdrawalServiceV1 {
    type Output = Result<(), ExecutionError>;

    /// This implementation is incorrect and instead of decrementing the balance of the wallet
    /// after withdraw, it increases it.
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

fn create_runtime() -> (BlockchainMut, EventsHandle) {
    let blockchain = Blockchain::build_for_tests();
    let genesis_config = create_genesis_config_builder()
        .with_artifact(ToySupervisorService.artifact_id())
        .with_instance(ToySupervisorService.default_instance())
        .with_artifact(WithdrawalServiceV1.artifact_id())
        .with_instance(WithdrawalServiceV1.default_instance())
        .build();

    let inspected = Inspected::new(
        RustRuntimeBuilder::new()
            .with_factory(WithdrawalServiceV1)
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
