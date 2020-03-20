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

use chrono::Utc;
use exonum_crypto::{Hash, KeyPair};
use exonum_derive::{BinaryValue, FromAccess};
use exonum_merkledb::{
    access::{Access, FromAccess},
    BinaryValue, Error as MerkledbError, ObjectHash, ProofListIndex, Snapshot, SystemSchema,
};
use pretty_assertions::assert_eq;
use semver::Version;

use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    panic,
};

use crate::{
    blockchain::{
        config::{ConsensusConfig, GenesisConfig, GenesisConfigBuilder, InstanceInitParams},
        BlockParams, Blockchain, BlockchainMut, PersistentPool, Schema, TransactionCache,
    },
    helpers::{Height, Round, ValidatorId},
    messages::{Precommit, Verified},
    runtime::{
        catch_panic,
        migrations::{InitMigrationError, MigrationScript},
        oneshot::Receiver,
        AnyTx, ArtifactId, CallInfo, CommonError, CoreError, Dispatcher, DispatcherSchema,
        ErrorMatch, ExecutionContext, ExecutionError, ExecutionFail, InstanceId, InstanceSpec,
        InstanceState, InstanceStatus, Mailbox, MethodId, Runtime, SnapshotExt, WellKnownRuntime,
        SUPERVISOR_INSTANCE_ID,
    },
};

const TEST_SERVICE_ID: InstanceId = SUPERVISOR_INSTANCE_ID;
const TEST_SERVICE_NAME: &str = "test_service";
const PANIC_STR: &str = "Panicking on request";

fn create_genesis_config() -> GenesisConfig {
    let (config, _) = ConsensusConfig::for_tests(1);
    GenesisConfigBuilder::with_consensus_config(config).build()
}

#[derive(Debug, FromAccess)]
struct InspectorSchema<T: Access> {
    values: ProofListIndex<T::Base, u64>,
}

impl<T: Access> InspectorSchema<T> {
    fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

/// Actions that performs at the `initiate_adding_service` stage.
#[derive(Debug, Clone, Serialize, Deserialize, BinaryValue)]
#[binary_value(codec = "bincode")]
enum InitAction {
    /// Nothing happens.
    Noop,
    /// Emit panic.
    Panic,
    /// Emit execution error with the corresponding code and description.
    Error(u8, String),
}

/// Describes action execution logic.
trait Execute {
    /// Executes the corresponding action.
    fn execute(self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError>;
}

impl InitAction {
    /// Creates a default instance init params.
    fn into_default_instance(self) -> InstanceInitParams {
        InstanceInitParams::new(
            TEST_SERVICE_ID,
            TEST_SERVICE_NAME,
            RuntimeInspector::default_artifact_id(),
            self,
        )
    }
}

impl Execute for InitAction {
    fn execute(self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        match self {
            Self::Noop => Ok(()),
            Self::Panic => panic!(PANIC_STR),
            Self::Error(code, description) => Err(ExecutionError::service(code, description)),
        }
    }
}

impl Default for InitAction {
    fn default() -> Self {
        Self::Noop
    }
}

/// Actions to be performed at the `after_transactions` stage.
#[derive(Debug, Clone)]
enum AfterTransactionsAction {
    /// Add some value to the inspector schema index.
    AddValue(u64),
    /// Emit panic.
    Panic,
}

impl Execute for AfterTransactionsAction {
    fn execute(self, context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        match self {
            Self::AddValue(value) => {
                let mut schema = InspectorSchema::new(&*context.fork);
                schema.values.push(value);
                Ok(())
            }

            Self::Panic => panic!(PANIC_STR),
        }
    }
}

/// Runtime inspector transaction set.
#[derive(Debug, Clone, Serialize, Deserialize, BinaryValue)]
#[binary_value(codec = "bincode")]
enum Transaction {
    /// Add some value to the inspector schema index.
    AddValue(u64),
    /// Emit panic.
    Panic,
    /// Emit MerkleDb error.
    MerkledbError,
    /// Emit execution error with the corresponding code and description.
    ExecutionError(u8, String),
    /// Deploy artifact with the specified ID.
    DeployArtifact(ArtifactId),
    /// Add service with the specified spec and init action.
    AddService(InstanceSpec, InitAction),
    /// Stop service with the specified ID.
    StopService(InstanceId),
}

impl Transaction {
    fn sign(self, instance_id: InstanceId, keypair: &KeyPair) -> Verified<AnyTx> {
        let tx = AnyTx::new(CallInfo::new(instance_id, 0), self.into_bytes());
        tx.sign_with_keypair(keypair)
    }
}

impl Execute for Transaction {
    fn execute(self, mut context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        match self {
            Self::AddValue(value) => {
                let mut schema = InspectorSchema::new(&*context.fork);
                schema.values.push(value);
                Ok(())
            }

            Self::Panic => {
                let mut schema = InspectorSchema::new(&*context.fork);
                schema.values.push(42);
                panic!(PANIC_STR);
            }

            Self::MerkledbError => panic!(MerkledbError::new(PANIC_STR)),

            Self::ExecutionError(code, description) => {
                let mut schema = InspectorSchema::new(&*context.fork);
                schema.values.push(42);
                Err(ExecutionError::service(code, description))
            }

            Self::DeployArtifact(artifact_id) => {
                // Code below will panic if there is already deployed artifact with the
                // same ID. This sort of expected behavior, since we're intentionally skipping
                // the `start_deploy` step (which will make the test nature much more complex).
                Dispatcher::commit_artifact(context.fork, &artifact_id, Vec::new());
                Ok(())
            }

            Self::AddService(spec, constructor) => {
                context.initiate_adding_service(spec, constructor)
            }

            Self::StopService(instance_id) => {
                Dispatcher::initiate_stopping_service(context.fork, instance_id)
            }
        }
    }
}

#[derive(Debug)]
struct RuntimeInspector {
    available: Vec<ArtifactId>,
    deployed: Vec<ArtifactId>,
    after_transactions: RefCell<VecDeque<AfterTransactionsAction>>,
}

impl WellKnownRuntime for RuntimeInspector {
    const ID: u32 = 255;
}

impl RuntimeInspector {
    fn empty() -> Self {
        Self {
            available: Vec::default(),
            deployed: Vec::default(),
            after_transactions: RefCell::default(),
        }
    }

    fn new(available: Vec<ArtifactId>) -> Self {
        Self {
            available,
            ..Self::default()
        }
    }

    fn with_available_artifact(mut self, artifact: ArtifactId) -> Self {
        self.available.push(artifact);
        self
    }

    fn with_after_transactions_action(mut self, action: AfterTransactionsAction) -> Self {
        self.after_transactions.get_mut().push_back(action);
        self
    }

    fn default_artifact_id() -> ArtifactId {
        ArtifactId::from_raw_parts(Self::ID, "runtime-inspector".into(), Version::new(1, 0, 0))
    }
}

impl Default for RuntimeInspector {
    fn default() -> Self {
        Self {
            available: vec![Self::default_artifact_id()],
            ..Self::empty()
        }
    }
}

impl Runtime for RuntimeInspector {
    fn deploy_artifact(&mut self, artifact: ArtifactId, _deploy_spec: Vec<u8>) -> Receiver {
        assert!(self.available.contains(&artifact));
        assert!(!self.deployed.contains(&artifact));

        self.deployed.push(artifact);

        Receiver::with_result(Ok(()))
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        self.deployed.contains(id)
    }

    fn initiate_adding_service(
        &self,
        context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        catch_panic(|| {
            InitAction::from_bytes(parameters.into())
                .map_err(|e| CommonError::MalformedArguments.with_description(e))?
                .execute(context)
        })
    }

    fn initiate_resuming_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn update_service_status(&mut self, _snapshot: &dyn Snapshot, _state: &InstanceState) {}

    fn migrate(
        &self,
        _new_artifact: &ArtifactId,
        _data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        Err(InitMigrationError::NotSupported)
    }

    fn execute(
        &self,
        context: ExecutionContext<'_>,
        _method_id: MethodId,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        catch_panic(|| {
            Transaction::from_bytes(arguments.into())
                .map_err(|e| CommonError::MalformedArguments.with_description(e))?
                .execute(context)
        })
    }

    fn before_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_transactions(&self, context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        catch_panic(|| {
            if let Some(action) = self.after_transactions.borrow_mut().pop_front() {
                action.execute(context)
            } else {
                Ok(())
            }
        })
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {}
}

// Attempts to create blockchain for particular Rust services and its instances assuming all of
// these are builtin services.
fn check_finalizing_services(artifacts: Vec<ArtifactId>, instances: Vec<InstanceInitParams>) {
    create_blockchain(RuntimeInspector::new(artifacts), instances);
}

fn execute_transaction(
    blockchain: &mut BlockchainMut,
    tx: Verified<AnyTx>,
) -> Result<(), ExecutionError> {
    let tx_hash = tx.object_hash();
    blockchain
        .merge({
            let fork = blockchain.fork();
            let mut schema = Schema::new(&fork);
            schema.add_transaction_into_pool(tx);
            fork.into_patch()
        })
        .unwrap();

    let epoch = blockchain.as_ref().last_block().epoch().unwrap().next();
    let patch = blockchain.create_patch(BlockParams::new(ValidatorId(0), epoch, &[tx_hash]), &());

    blockchain.commit(patch, vec![]).unwrap();
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    let location = schema.transactions_locations().get(&tx_hash).unwrap();
    schema.transaction_result(location).unwrap()
}

/// Attempts to create a blockchain, returning an error if the genesis block
/// was not created.
fn create_blockchain(
    runtime: RuntimeInspector,
    instances: Vec<InstanceInitParams>,
) -> BlockchainMut {
    let (config, _) = ConsensusConfig::for_tests(1);
    let genesis_config = instances
        .into_iter()
        .fold(
            GenesisConfigBuilder::with_consensus_config(config),
            |builder, instance| {
                builder
                    .with_artifact(instance.instance_spec.artifact.clone())
                    .with_instance(instance)
            },
        )
        .build();

    Blockchain::build_for_tests()
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
}

/// Checks that `after_transactions` is invoked for services added
/// within genesis block.
#[test]
fn after_transactions_invoked_on_genesis() {
    // Set the value in schema within `after_transactions`.
    let blockchain = create_blockchain(
        RuntimeInspector::default()
            .with_after_transactions_action(AfterTransactionsAction::AddValue(1)),
        vec![InitAction::Noop.into_default_instance()],
    );

    // After creation of the genesis block, check that value was set.
    let snapshot = blockchain.snapshot();
    let schema = InspectorSchema::new(&snapshot);

    assert_eq!(schema.values.len(), 1);
    assert_eq!(schema.values.get(0), Some(1));
}

/// Checks that if `after_transactions` fails on the genesis block,
/// the blockchain is not created.
#[test]
#[should_panic(expected = "Panicking on request")]
fn after_transactions_failure_causes_genesis_failure() {
    create_blockchain(
        RuntimeInspector::default().with_after_transactions_action(AfterTransactionsAction::Panic),
        vec![InitAction::Noop.into_default_instance()],
    );
}

#[test]
fn handling_tx_panic_error() {
    let keys = KeyPair::random();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );

    // Check that failed transactions do not modify inspector schema.
    let failed_transactions = vec![
        (Transaction::Panic, PANIC_STR),
        (
            Transaction::ExecutionError(0, "Service error".to_owned()),
            "Service error",
        ),
    ];

    for (tx, expected_err) in failed_transactions {
        let actual_err = execute_transaction(&mut blockchain, tx.sign(TEST_SERVICE_ID, &keys))
            .expect_err("Transaction must fail");

        assert_eq!(actual_err.description(), expected_err);
        let snapshot = blockchain.snapshot();
        assert!(
            InspectorSchema::new(&snapshot).values.is_empty(),
            "Changes in the schema should be discarded"
        );
    }

    // Check that the transaction modifies inspector schema.
    execute_transaction(
        &mut blockchain,
        Transaction::AddValue(10).sign(TEST_SERVICE_ID, &keys),
    )
    .expect("Transaction must success");

    let snapshot = blockchain.snapshot();
    assert_eq!(InspectorSchema::new(&snapshot).values.get(0), Some(10));
}

#[test]
#[should_panic]
fn handling_tx_merkledb_error() {
    let keys = KeyPair::random();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );

    execute_transaction(
        &mut blockchain,
        Transaction::MerkledbError.sign(TEST_SERVICE_ID, &keys),
    )
    .unwrap();
}

#[test]
fn initialize_service_ok() {
    create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );
}

#[test]
fn deploy_available() {
    let keys = KeyPair::random();

    let artifact_id = ArtifactId::from_raw_parts(
        RuntimeInspector::ID,
        "secondary".into(),
        Version::new(1, 0, 0),
    );
    let mut blockchain = create_blockchain(
        RuntimeInspector::default().with_available_artifact(artifact_id.clone()),
        vec![InitAction::Noop.into_default_instance()],
    );

    execute_transaction(
        &mut blockchain,
        Transaction::DeployArtifact(artifact_id).sign(TEST_SERVICE_ID, &keys),
    )
    .unwrap();
}

#[test]
fn deploy_already_deployed() {
    let keys = KeyPair::random();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );

    let actual_err = execute_transaction(
        &mut blockchain,
        Transaction::DeployArtifact(RuntimeInspector::default_artifact_id())
            .sign(TEST_SERVICE_ID, &keys),
    )
    .unwrap_err();

    // Since `RuntimeInspector` transactions skip the `start_deploy`,
    // we expect transaction to panic (`commit_service` is called within transaction body).
    let expect_err = ErrorMatch::any_unexpected()
        .with_description_containing("Cannot deploy artifact `255:runtime-inspector:1.0.0` twice");
    assert_eq!(actual_err, expect_err);
}

#[test]
#[should_panic(expected = "assertion failed: self.available.contains(&artifact)")]
fn deploy_unavailable_artifact() {
    let keys = KeyPair::random();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );

    let artifact_id = ArtifactId::from_raw_parts(
        RuntimeInspector::ID,
        "secondary".into(),
        Version::new(1, 0, 0),
    );
    execute_transaction(
        &mut blockchain,
        Transaction::DeployArtifact(artifact_id).sign(TEST_SERVICE_ID, &keys),
    )
    .unwrap_err();
}

#[test]
fn start_stop_service_instance() {
    let keys = KeyPair::random();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );

    // Start secondary service instance.
    let instance_spec = InstanceSpec::from_raw_parts(
        10,
        "secondary".to_owned(),
        RuntimeInspector::default_artifact_id(),
    );

    // Check that the secondary service instance is absent in the dispatcher schema.
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .instances()
        .contains(&instance_spec.name));

    execute_transaction(
        &mut blockchain,
        Transaction::AddService(instance_spec.clone(), InitAction::Noop)
            .sign(TEST_SERVICE_ID, &keys),
    )
    .unwrap();

    // Check that the service status in the dispatcher schema is active.
    let snapshot = blockchain.snapshot();
    assert_eq!(
        DispatcherSchema::new(&snapshot)
            .instances()
            .get(&instance_spec.name)
            .unwrap()
            .status,
        Some(InstanceStatus::Active)
    );

    // Stop another service instance.
    execute_transaction(
        &mut blockchain,
        Transaction::StopService(instance_spec.id).sign(TEST_SERVICE_ID, &keys),
    )
    .unwrap();
    // Check that the service status in the dispatcher schema is stopped.
    let snapshot = blockchain.snapshot();
    assert_eq!(
        DispatcherSchema::new(&snapshot)
            .instances()
            .get(&instance_spec.name)
            .unwrap()
            .status,
        Some(InstanceStatus::Stopped)
    );
}

/// Checks that `Blockchain::check_tx` discards transactions with incorrect
/// instance IDs.
#[test]
fn test_check_tx() {
    let keys = KeyPair::random();
    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );

    let snapshot = blockchain.snapshot();
    let correct_tx = Transaction::AddValue(1).sign(TEST_SERVICE_ID, &keys);
    Blockchain::check_tx(&snapshot, &correct_tx).expect("Correct transaction");

    let incorrect_tx = Transaction::AddValue(1).sign(TEST_SERVICE_ID + 1, &keys);
    let unknown_msg = "Cannot dispatch transaction to unknown service with ID 1";
    assert_eq!(
        Blockchain::check_tx(&snapshot, &incorrect_tx).unwrap_err(),
        ErrorMatch::from_fail(&CoreError::IncorrectInstanceId)
            .with_description_containing(unknown_msg)
    );

    // Stop service instance to make correct_tx incorrect.
    execute_transaction(
        &mut blockchain,
        Transaction::StopService(TEST_SERVICE_ID).sign(TEST_SERVICE_ID, &keys),
    )
    .expect("Correct transaction");

    // Check that previously correct transaction become incorrect.
    let snapshot = blockchain.snapshot();
    let not_active_msg = "Cannot dispatch transaction to non-active service";
    assert_eq!(
        Blockchain::check_tx(&snapshot, &correct_tx).unwrap_err(),
        ErrorMatch::from_fail(&CoreError::ServiceNotActive)
            .with_description_containing(not_active_msg)
    );
}

#[test]
#[should_panic(expected = "Service with name `sample_instance` already exists")]
fn finalize_duplicate_services() {
    let artifact = RuntimeInspector::default_artifact_id();
    let instance = InstanceInitParams::new(
        10,
        "sample_instance",
        artifact.clone(),
        InitAction::default(),
    );

    check_finalizing_services(vec![artifact], vec![instance.clone(), instance]);
}

#[test]
#[should_panic(expected = "Service with name `sample_instance` already exists")]
fn finalize_services_with_duplicate_names() {
    let artifact = RuntimeInspector::default_artifact_id();

    let instances = vec![
        InstanceInitParams::new(
            10,
            "sample_instance",
            artifact.clone(),
            InitAction::default(),
        ),
        InstanceInitParams::new(
            11,
            "sample_instance",
            artifact.clone(),
            InitAction::default(),
        ),
    ];

    check_finalizing_services(vec![artifact], instances);
}

#[test]
#[should_panic(expected = "Service with numeric ID 10 already exists")]
fn finalize_services_with_duplicate_ids() {
    let artifact = RuntimeInspector::default_artifact_id();

    let instances = vec![
        InstanceInitParams::new(
            10,
            "sample_instance",
            artifact.clone(),
            InitAction::default(),
        ),
        InstanceInitParams::new(
            10,
            "sample_instance_2",
            artifact.clone(),
            InitAction::default(),
        ),
    ];

    check_finalizing_services(vec![artifact], instances);
}

/// Checks that before genesis creation `Schema::height` panics.
#[test]
#[should_panic(
    expected = "An attempt to get the actual `height` during creating the genesis block"
)]
fn blockchain_height_panics_before_genesis() {
    // Create a blockchain *without* creating a genesis block.
    let blockchain = Blockchain::build_for_tests();

    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    let _height = schema.height();
}

/// Checks that before genesis creation `Schema::next_height` doesn't panic.
#[test]
fn blockchain_next_height_does_not_panic_before_genesis() {
    // Create a blockchain *without* creating a genesis block.
    let blockchain = Blockchain::build_for_tests();

    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    let height = schema.next_height();
    assert_eq!(height, Height(0))
}

/// Checks that `Schema::height` and `Schema::next_height` work as expected.
#[test]
fn blockchain_height() {
    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(create_genesis_config())
        .build();

    // Check that height is 0 after genesis creation.
    let snapshot = blockchain.snapshot();
    let schema = snapshot.for_core();
    assert_eq!(schema.height(), Height(0));
    assert_eq!(schema.next_height(), Height(1));

    // Create one block.
    let block_params = BlockParams::new(ValidatorId(0), Height(1), &[]);
    let patch = blockchain.create_patch(block_params, &());
    blockchain.merge(patch.into_inner()).unwrap();

    // Check that height is 1.
    let snapshot = blockchain.snapshot();
    let schema = snapshot.for_core();
    assert_eq!(schema.height(), Height(1));
    assert_eq!(schema.next_height(), Height(2));
}

#[test]
fn state_aggregation() {
    let keys = KeyPair::random();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );

    execute_transaction(
        &mut blockchain,
        Transaction::AddValue(10).sign(TEST_SERVICE_ID, &keys),
    )
    .expect("Transaction must succeed");

    let snapshot = blockchain.snapshot();
    let expected_indexes = vec![
        "core.consensus_config",
        "dispatcher_artifacts",
        "dispatcher_instances",
        "values",
    ];
    let actual_indexes: Vec<_> = SystemSchema::new(&snapshot)
        .state_aggregator()
        .keys()
        .collect();
    assert_eq!(actual_indexes, expected_indexes);
}

#[test]
fn no_data_race_for_transaction_pool() {
    let keys = KeyPair::random();
    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );

    let tx = Transaction::AddValue(10).sign(TEST_SERVICE_ID, &keys);
    let tx_hash = tx.object_hash();
    let mut tx_cache = BTreeMap::new();
    tx_cache.insert(tx_hash, tx.clone());

    let patch = blockchain.create_patch(
        BlockParams::new(ValidatorId(0), Height(1), &[tx_hash]),
        &tx_cache,
    );

    let snapshot = blockchain.snapshot();
    let is_known = PersistentPool::new(&snapshot, &tx_cache).contains_transaction(tx_hash);
    assert!(is_known);
    let schema = Schema::new(&snapshot);
    let is_in_pool =
        schema.transactions_pool().contains(&tx_hash) || tx_cache.contains_key(&tx_hash);
    assert!(is_in_pool);

    // Move transaction to persistent pool while the block is being accepted.
    let fork = blockchain.fork();
    Schema::new(&fork).add_transaction_into_pool(tx);
    blockchain.merge(fork.into_patch()).unwrap();

    // Accept the block and check that the core schema remains logically consistent.
    blockchain.commit(patch, vec![]).unwrap();
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    assert_eq!(schema.transactions_len(), 1);
    assert_eq!(schema.transactions_pool_len(), 0);
}

#[test]
fn executing_block_skip() {
    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );
    let block_params = BlockParams::skip(ValidatorId(0), Height(1));
    let patch = blockchain.create_patch(block_params, &());

    let validator_keys = KeyPair::random();
    let precommit = Precommit {
        validator: ValidatorId(0),
        epoch: Height(1),
        round: Round(1),
        propose_hash: Hash::zero(),
        block_hash: patch.block_hash(),
        time: Utc::now(),
    };
    let precommit = Verified::from_value(
        precommit,
        validator_keys.public_key(),
        validator_keys.secret_key(),
    );
    blockchain.commit(patch, vec![precommit]).unwrap();

    // Check that the blockchain height remained the same.
    let last_block = blockchain.as_ref().last_block();
    assert_eq!(last_block.height, Height(0));
    assert_eq!(last_block.object_hash(), blockchain.as_ref().last_hash());

    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    let block_proof = schema.block_skip_and_precommits().unwrap();
    assert_eq!(block_proof.block.height, Height(0));
    assert_eq!(block_proof.block.prev_hash, last_block.object_hash());
    assert_eq!(block_proof.block.epoch(), Some(Height(1)));
    assert_eq!(block_proof.precommits.len(), 1);
    block_proof.verify(&[validator_keys.public_key()]).unwrap();
}

#[test]
fn clearing_block_skip() {
    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    );
    let block_params = BlockParams::skip(ValidatorId(0), Height(1));
    let patch = blockchain.create_patch(block_params, &());
    let block_hash = patch.block_hash();

    let validator_keys = KeyPair::random();
    let mut precommit_payload = Precommit {
        validator: ValidatorId(0),
        epoch: Height(1),
        round: Round(1),
        propose_hash: Hash::zero(),
        block_hash,
        time: Utc::now(),
    };
    let precommit = Verified::from_value(
        precommit_payload.clone(),
        validator_keys.public_key(),
        validator_keys.secret_key(),
    );
    blockchain.commit(patch, vec![precommit]).unwrap();

    // Commit a new block skip.
    let block_params = BlockParams::skip(ValidatorId(0), Height(2));
    let patch = blockchain.create_patch(block_params, &());
    let new_block_hash = patch.block_hash();
    assert_ne!(new_block_hash, block_hash);
    precommit_payload.block_hash = new_block_hash;
    precommit_payload.epoch = Height(2);

    let precommit = Verified::from_value(
        precommit_payload,
        validator_keys.public_key(),
        validator_keys.secret_key(),
    );
    blockchain.commit(patch, vec![precommit]).unwrap();

    // Check that the new block skip is correct.
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    let block_proof = schema.block_skip_and_precommits().unwrap();
    assert_eq!(block_proof.block.height, Height(0));
    assert_eq!(block_proof.block.epoch(), Some(Height(2)));
    assert_eq!(block_proof.precommits.len(), 1);
    block_proof.verify(&[validator_keys.public_key()]).unwrap();

    // Check that the old precommit has been erased.
    assert!(schema.precommits(&block_hash).is_empty());

    // Commit an ordinary block.
    execute_transaction(
        &mut blockchain,
        Transaction::AddValue(10).sign(TEST_SERVICE_ID, &validator_keys),
    )
    .expect("Transaction must succeed");

    // Check that the block skip is now erased together with its precommits.
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    assert!(schema.block_skip().is_none());
    assert!(schema.precommits(&new_block_hash).is_empty());
}
