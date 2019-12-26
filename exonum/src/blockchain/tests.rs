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

use exonum_crypto::{PublicKey, SecretKey};
use exonum_derive::FromAccess;
use exonum_merkledb::{
    access::Access, BinaryValue, Error as MerkledbError, ObjectHash, ProofListIndex, Snapshot,
    SystemSchema,
};
use futures::{Future, IntoFuture};
use semver::Version;

use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    panic,
};

use crate::{
    blockchain::{
        config::{ConsensusConfig, GenesisConfig, GenesisConfigBuilder, InstanceInitParams},
        Blockchain, BlockchainBuilder, BlockchainMut, Schema,
    },
    helpers::{generate_testnet_config, Height, ValidatorId},
    messages::Verified,
    runtime::{
        catch_panic, AnyTx, ArtifactId, CallInfo, Dispatcher, DispatcherError, DispatcherSchema,
        ErrorMatch, ExecutionContext, ExecutionError, ExecutionFail, InstanceId, InstanceSpec,
        InstanceStatus, Mailbox, Runtime, SnapshotExt, WellKnownRuntime, SUPERVISOR_INSTANCE_ID,
    },
};

const TEST_SERVICE_ID: InstanceId = SUPERVISOR_INSTANCE_ID;
const TEST_SERVICE_NAME: &str = "test_service";
const PANIC_STR: &str = "42";

macro_rules! impl_binary_value_for_bincode {
    ($( $type:ty ),*) => {
        $(
            impl BinaryValue for $type {
                fn to_bytes(&self) -> Vec<u8> {
                    bincode::serialize(self).expect("Error while serializing value")
                }
                fn from_bytes(bytes: std::borrow::Cow<'_, [u8]>) -> Result<Self, failure::Error> {
                    bincode::deserialize(bytes.as_ref()).map_err(From::from)
                }
            }
        )*
    };
}

fn create_consensus_config() -> ConsensusConfig {
    generate_testnet_config(1, 0)[0].clone().consensus
}

fn create_genesis_config() -> GenesisConfig {
    GenesisConfigBuilder::with_consensus_config(create_consensus_config()).build()
}

#[derive(Debug, FromAccess)]
struct InspectorSchema<T: Access> {
    values: ProofListIndex<T::Base, u64>,
}

/// Actions that performs at the `initiate_adding_service` stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            InitAction::Noop => Ok(()),

            InitAction::Panic => panic!(PANIC_STR),

            InitAction::Error(code, description) => Err(ExecutionError::service(code, description)),
        }
    }
}

impl Default for InitAction {
    fn default() -> Self {
        InitAction::Noop
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
            AfterTransactionsAction::AddValue(value) => {
                let mut schema = InspectorSchema::new(&*context.fork);
                schema.values.push(value);
                Ok(())
            }

            AfterTransactionsAction::Panic => panic!(PANIC_STR),
        }
    }
}

/// Runtime inspector transaction set.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    fn sign(
        self,
        instance_id: InstanceId,
        public_key: PublicKey,
        secret_key: &SecretKey,
    ) -> Verified<AnyTx> {
        let tx = AnyTx {
            arguments: self.into_bytes(),
            call_info: CallInfo::new(instance_id, 0),
        };
        Verified::from_value(tx, public_key, secret_key)
    }
}

impl Execute for Transaction {
    fn execute(self, mut context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        match self {
            Transaction::AddValue(value) => {
                let mut schema = InspectorSchema::new(&*context.fork);
                schema.values.push(value);
                Ok(())
            }

            Transaction::Panic => {
                let mut schema = InspectorSchema::new(&*context.fork);
                schema.values.push(42);
                panic!(PANIC_STR);
            }

            Transaction::MerkledbError => panic!(MerkledbError::new(PANIC_STR)),

            Transaction::ExecutionError(code, description) => {
                let mut schema = InspectorSchema::new(&*context.fork);
                schema.values.push(42);
                Err(ExecutionError::service(code, description))
            }

            Transaction::DeployArtifact(artifact_id) => {
                Dispatcher::commit_artifact(&*context.fork, artifact_id, Vec::new())
            }

            Transaction::AddService(spec, constructor) => {
                context.initiate_adding_service(spec, constructor)
            }

            Transaction::StopService(instance_id) => {
                Dispatcher::initiate_stopping_service(&*context.fork, instance_id)
            }
        }
    }
}

impl_binary_value_for_bincode! { InitAction, Transaction }

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
            ..Default::default()
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
        ArtifactId::new(Self::ID, "runtime-inspector", Version::new(1, 0, 0)).unwrap()
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
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        _deploy_spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        assert!(self.available.contains(&artifact));
        self.deployed.push(artifact);
        Box::new(Ok(()).into_future())
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        self.deployed.contains(id)
    }

    fn initiate_adding_service(
        &self,
        context: ExecutionContext<'_>,
        _spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        catch_panic(|| {
            InitAction::from_bytes(parameters.into())
                .map_err(|e| DispatcherError::MalformedArguments.with_description(e))?
                .execute(context)
        })
    }

    fn update_service_status(
        &mut self,
        _snapshot: &dyn Snapshot,
        _spec: &InstanceSpec,
        _status: InstanceStatus,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn execute(
        &self,
        context: ExecutionContext<'_>,
        _call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        catch_panic(|| {
            Transaction::from_bytes(arguments.into())
                .map_err(|e| DispatcherError::MalformedArguments.with_description(e))?
                .execute(context)
        })
    }

    fn before_transactions(
        &self,
        _context: ExecutionContext<'_>,
        _instance_id: InstanceId,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_transactions(
        &self,
        context: ExecutionContext<'_>,
        _instance_id: InstanceId,
    ) -> Result<(), ExecutionError> {
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
    create_blockchain(RuntimeInspector::new(artifacts), instances).unwrap();
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

    let height = {
        let snapshot = blockchain.snapshot();
        Schema::new(&snapshot).next_height()
    };

    let (block_hash, patch) = blockchain.create_patch(
        ValidatorId::zero().into(),
        height,
        &[tx_hash],
        &mut BTreeMap::new(),
    );

    blockchain
        .commit(patch, block_hash, vec![], &mut BTreeMap::new())
        .unwrap();
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
) -> Result<BlockchainMut, failure::Error> {
    let genesis_config = instances
        .into_iter()
        .fold(
            GenesisConfigBuilder::with_consensus_config(create_consensus_config()),
            |builder, instance| {
                builder
                    .with_artifact(instance.instance_spec.artifact.clone())
                    .with_instance(instance)
            },
        )
        .build();

    BlockchainBuilder::new(Blockchain::build_for_tests(), genesis_config)
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
    )
    .unwrap();

    // After creation of the genesis block, check that value was set.
    let snapshot = blockchain.snapshot();
    let schema = InspectorSchema::new(&snapshot);

    assert_eq!(schema.values.len(), 1);
    assert_eq!(schema.values.get(0), Some(1));
}

/// Checks that if `after_transactions` fails on the genesis block,
/// the blockchain is not created.
#[test]
fn after_transactions_failure_causes_genesis_failure() {
    let actual_err = create_blockchain(
        RuntimeInspector::default().with_after_transactions_action(AfterTransactionsAction::Panic),
        vec![InitAction::Noop.into_default_instance()],
    )
    .unwrap_err();

    // Unfortunately, `failure::Error` doesn't implement `PartialEq`, so we have to string-compare them.
    assert!(
        actual_err.to_string().contains(PANIC_STR),
        "Expected error should be caused by `after_transactions` hook"
    );
}

#[test]
fn handling_tx_panic_error() {
    let (pk, sk) = exonum_crypto::gen_keypair();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    )
    .unwrap();

    // Check that failed transactions do not modify inspector schema.
    let failed_transactions = vec![
        (Transaction::Panic, PANIC_STR),
        (
            Transaction::ExecutionError(0, "Service error".to_owned()),
            "Service error",
        ),
    ];

    for (tx, expected_err) in failed_transactions {
        let actual_err = execute_transaction(&mut blockchain, tx.sign(TEST_SERVICE_ID, pk, &sk))
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
        Transaction::AddValue(10).sign(TEST_SERVICE_ID, pk, &sk),
    )
    .expect("Transaction must success");

    let snapshot = blockchain.snapshot();
    assert_eq!(InspectorSchema::new(&snapshot).values.get(0), Some(10));
}

#[test]
#[should_panic]
fn handling_tx_merkledb_error() {
    let (pk, sk) = exonum_crypto::gen_keypair();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    )
    .unwrap();

    execute_transaction(
        &mut blockchain,
        Transaction::MerkledbError.sign(TEST_SERVICE_ID, pk, &sk),
    )
    .unwrap();
}

#[test]
fn initialize_service_ok() {
    create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    )
    .unwrap();
}

#[test]
fn deploy_available() {
    let (pk, sk) = exonum_crypto::gen_keypair();

    let artifact_id =
        ArtifactId::new(RuntimeInspector::ID, "secondary", Version::new(1, 0, 0)).unwrap();
    let mut blockchain = create_blockchain(
        RuntimeInspector::default().with_available_artifact(artifact_id.clone()),
        vec![InitAction::Noop.into_default_instance()],
    )
    .unwrap();

    execute_transaction(
        &mut blockchain,
        Transaction::DeployArtifact(artifact_id).sign(TEST_SERVICE_ID, pk, &sk),
    )
    .unwrap();
}

#[test]
fn deploy_already_deployed() {
    let (pk, sk) = exonum_crypto::gen_keypair();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    )
    .unwrap();

    let actual_err = execute_transaction(
        &mut blockchain,
        Transaction::DeployArtifact(RuntimeInspector::default_artifact_id()).sign(
            TEST_SERVICE_ID,
            pk,
            &sk,
        ),
    )
    .unwrap_err();

    let expect_err = ErrorMatch::from_fail(&DispatcherError::ArtifactAlreadyDeployed);
    assert_eq!(actual_err, expect_err);
}

#[test]
#[should_panic(expected = "assertion failed: self.available.contains(&artifact)")]
fn deploy_unavailable_artifact() {
    let (pk, sk) = exonum_crypto::gen_keypair();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    )
    .unwrap();

    let artifact_id =
        ArtifactId::new(RuntimeInspector::ID, "secondary", Version::new(1, 0, 0)).unwrap();
    execute_transaction(
        &mut blockchain,
        Transaction::DeployArtifact(artifact_id).sign(TEST_SERVICE_ID, pk, &sk),
    )
    .unwrap_err();
}

#[test]
fn start_stop_service_instance() {
    let (pk, sk) = exonum_crypto::gen_keypair();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    )
    .unwrap();

    // Start secondary service instance.
    let instance_spec = InstanceSpec {
        id: 10,
        name: "secondary".to_owned(),
        artifact: RuntimeInspector::default_artifact_id(),
    };

    // Check that the secondary service instance is absent in the dispatcher schema.
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .instances()
        .contains(&instance_spec.name));

    execute_transaction(
        &mut blockchain,
        Transaction::AddService(instance_spec.clone(), InitAction::Noop).sign(
            TEST_SERVICE_ID,
            pk,
            &sk,
        ),
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
        Transaction::StopService(instance_spec.id).sign(TEST_SERVICE_ID, pk, &sk),
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
    let (pk, sk) = exonum_crypto::gen_keypair();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    )
    .unwrap();

    let snapshot = blockchain.snapshot();

    let correct_tx = Transaction::AddValue(1).sign(TEST_SERVICE_ID, pk, &sk);
    Blockchain::check_tx(&snapshot, &correct_tx).expect("Correct transaction");

    let incorrect_tx = Transaction::AddValue(1).sign(TEST_SERVICE_ID + 1, pk, &sk);
    assert_eq!(
        Blockchain::check_tx(&snapshot, &incorrect_tx).expect_err("Incorrect transaction"),
        ErrorMatch::from_fail(&DispatcherError::IncorrectInstanceId)
    );

    // Stop service instance to make correct_tx incorrect.
    execute_transaction(
        &mut blockchain,
        Transaction::StopService(TEST_SERVICE_ID).sign(TEST_SERVICE_ID, pk, &sk),
    )
    .expect("Correct transaction");

    // Check that previously correct transaction become incorrect.
    let snapshot = blockchain.snapshot();
    assert_eq!(
        Blockchain::check_tx(&snapshot, &correct_tx).unwrap_err(),
        ErrorMatch::from_fail(&DispatcherError::ServiceNotActive)
    );
}

#[test]
#[should_panic(expected = "already used")]
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
#[should_panic(expected = "already used")]
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
#[should_panic(expected = "already used")]
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
    let mut blockchain =
        BlockchainBuilder::new(Blockchain::build_for_tests(), create_genesis_config())
            .build()
            .unwrap();

    // Check that height is 0 after genesis creation.
    let snapshot = blockchain.snapshot();
    let schema = snapshot.for_core();
    assert_eq!(schema.height(), Height(0));
    assert_eq!(schema.next_height(), Height(1));

    // Create one block.
    let (_, patch) = blockchain.create_patch(
        ValidatorId::zero().into(),
        Height::zero(),
        &[],
        &mut BTreeMap::new(),
    );
    blockchain.merge(patch).unwrap();

    // Check that height is 1.
    let snapshot = blockchain.snapshot();
    let schema = snapshot.for_core();
    assert_eq!(schema.height(), Height(1));
    assert_eq!(schema.next_height(), Height(2));
}

#[test]
fn state_aggregation() {
    let (pk, sk) = exonum_crypto::gen_keypair();

    let mut blockchain = create_blockchain(
        RuntimeInspector::default(),
        vec![InitAction::Noop.into_default_instance()],
    )
    .unwrap();

    execute_transaction(
        &mut blockchain,
        Transaction::AddValue(10).sign(TEST_SERVICE_ID, pk, &sk),
    )
    .expect("Transaction must success");

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
