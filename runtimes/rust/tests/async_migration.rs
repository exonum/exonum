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

use assert_matches::assert_matches;
use exonum::{
    blockchain::{ApiSender, Blockchain, BlockchainBuilder, BlockchainMut},
    crypto::KeyPair,
    helpers::Height,
    merkledb::{
        access::{Access, CopyAccessExt, FromAccess},
        Database, ObjectHash, ProofEntry, TemporaryDB,
    },
    runtime::{
        migrations::{
            InitMigrationError, MigrateData, MigrationContext, MigrationError, MigrationScript,
        },
        versioning::{ArtifactReqError, Version},
        ArtifactId, CoreError, ErrorMatch, ExecutionContext, ExecutionError, InstanceId,
        InstanceStatus, RuntimeIdentifier, SnapshotExt,
    },
};
use exonum_api::UpdateEndpoints;
use exonum_derive::*;
use futures::channel::mpsc;
use pretty_assertions::assert_eq;

use std::{cmp, sync::Arc};

use self::inspected::{
    assert_no_endpoint_update, create_genesis_config_builder, execute_transaction,
    get_endpoint_paths, CommitMigration, EventsHandle, Inspected, MigrateService, ResumeService,
    RuntimeEvent, ToySupervisor, ToySupervisorService,
};
use exonum_rust_runtime::{
    spec::{Deploy, Spec},
    ArtifactProtobufSpec, DefaultInstance, RustRuntimeBuilder, Service, ServiceFactory,
};

mod inspected;

/// Artifact versions initially deployed on the blockchain.
const VERSIONS: &[&str] = &["0.1.0", "0.1.1", "0.1.5", "0.2.0"];

impl CommitMigration {
    fn for_counter(blockchain: &BlockchainMut, new_counter_value: u64) -> Self {
        let migration_hash = {
            let fork = blockchain.fork();
            let mut aggregator = fork.get_proof_map("_temp");
            aggregator.put("counter.counter", new_counter_value.object_hash());
            aggregator.object_hash()
        };
        Self {
            instance_name: CounterFactory::INSTANCE_NAME.to_owned(),
            migration_hash,
        }
    }
}

#[derive(Debug, FromAccess, RequireArtifact)]
#[require_artifact(name = "counter", version = "^0.1")]
struct Schema<T: Access> {
    counter: ProofEntry<T::Base, u64>,
}

impl<T: Access> Schema<T> {
    pub fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

#[exonum_interface(auto_ids)]
trait CountInterface<Ctx> {
    type Output;

    fn increment(&self, context: Ctx, value: u64) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher)]
#[service_dispatcher(implements("CountInterface"))]
struct Counter;

impl Service for Counter {}

impl CountInterface<ExecutionContext<'_>> for Counter {
    type Output = Result<(), ExecutionError>;

    fn increment(&self, context: ExecutionContext<'_>, value: u64) -> Self::Output {
        let mut schema = Schema::new(context.service_data());
        let count = schema.counter.get().unwrap_or(0);
        schema.counter.set(count + value);
        Ok(())
    }
}

#[derive(Debug)]
struct CounterFactory {
    version: Version,
}

impl CounterFactory {
    fn new(version: Version) -> Self {
        Self { version }
    }
}

impl ServiceFactory for CounterFactory {
    fn artifact_id(&self) -> ArtifactId {
        ArtifactId::from_raw_parts(
            RuntimeIdentifier::Rust as _,
            "counter".to_owned(),
            self.version.clone(),
        )
    }

    fn artifact_protobuf_spec(&self) -> ArtifactProtobufSpec {
        ArtifactProtobufSpec::default()
    }

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(Counter)
    }
}

impl DefaultInstance for CounterFactory {
    const INSTANCE_ID: InstanceId = 100;
    const INSTANCE_NAME: &'static str = "counter";
}

fn migration_script(context: &mut MigrationContext) -> Result<(), MigrationError> {
    let old_schema = Schema::new(context.helper.old_data());
    let mut new_schema = Schema::new(context.helper.new_data());
    new_schema
        .counter
        .set(old_schema.counter.get().unwrap_or(0) + 1);
    Ok(())
}

impl MigrateData for CounterFactory {
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        // We use custom implementation here to generate an infinite amount of scripts.
        let min_version = Version::new(0, 1, 0);
        let max_version = cmp::min(Version::new(0, 2, 0), self.version.clone());
        if *start_version < min_version {
            Err(InitMigrationError::OldStartVersion {
                min_supported_version: min_version,
            })
        } else if *start_version > max_version {
            Err(InitMigrationError::FutureStartVersion {
                max_supported_version: max_version,
            })
        } else if *start_version == max_version {
            Ok(vec![])
        } else {
            let mut end_version = start_version.to_owned();
            end_version.increment_patch();
            let script = MigrationScript::new(migration_script, end_version);
            Ok(vec![script])
        }
    }
}

fn create_runtime(
    db: impl Into<Arc<dyn Database>>,
) -> (BlockchainMut, EventsHandle, mpsc::Receiver<UpdateEndpoints>) {
    let mut counter_services = VERSIONS.iter().map(|&version| {
        let factory = CounterFactory::new(version.parse().unwrap());
        Spec::migrating(factory)
    });

    let mut genesis = create_genesis_config_builder();
    let mut rust_runtime = RustRuntimeBuilder::new();
    Spec::new(ToySupervisorService)
        .with_default_instance()
        .deploy(&mut genesis, &mut rust_runtime);

    // Deploy the instance of the earliest counter service and artifacts for other versions.
    let service = counter_services.next().unwrap();
    service
        .with_default_instance()
        .deploy(&mut genesis, &mut rust_runtime);
    for service in counter_services {
        service.deploy(&mut genesis, &mut rust_runtime);
    }

    let (endpoints_tx, endpoints_rx) = mpsc::channel(16);
    let inspected = Inspected::new(rust_runtime.build(endpoints_tx));
    let events_handle = inspected.events.clone();

    let blockchain = Blockchain::new(db, KeyPair::random(), ApiSender::closed());
    let blockchain = BlockchainBuilder::new(blockchain)
        .with_genesis_config(genesis.build())
        .with_runtime(inspected)
        .build();
    (blockchain, events_handle, endpoints_rx)
}

fn test_basic_migration(freeze_service: bool) {
    let (mut blockchain, events, mut endpoints_rx) = create_runtime(TemporaryDB::new());
    let old_spec = CounterFactory::new(VERSIONS[0].parse().unwrap())
        .default_instance()
        .instance_spec;
    let new_artifact = CounterFactory::new(VERSIONS[2].parse().unwrap()).artifact_id();
    get_endpoint_paths(&mut endpoints_rx);

    let keypair = KeyPair::random();
    let tx = keypair.increment(CounterFactory::INSTANCE_ID, 1);
    execute_transaction(&mut blockchain, tx).unwrap();
    assert_no_endpoint_update(&mut endpoints_rx);

    // Stop or freeze the service.
    let tx = if freeze_service {
        keypair.freeze_service(
            ToySupervisorService::INSTANCE_ID,
            CounterFactory::INSTANCE_ID,
        )
    } else {
        keypair.stop_service(
            ToySupervisorService::INSTANCE_ID,
            CounterFactory::INSTANCE_ID,
        )
    };
    execute_transaction(&mut blockchain, tx).unwrap();

    if freeze_service {
        assert_no_endpoint_update(&mut endpoints_rx);
    } else {
        let paths = get_endpoint_paths(&mut endpoints_rx);
        assert!(paths.contains("services/supervisor"));
        assert!(!paths.contains("services/counter"));
    }

    // Start async migration.
    let migration = MigrateService {
        instance_name: CounterFactory::INSTANCE_NAME.to_owned(),
        artifact: new_artifact,
    };
    let tx = keypair.migrate_service(ToySupervisorService::INSTANCE_ID, migration);
    execute_transaction(&mut blockchain, tx).unwrap();

    if freeze_service {
        assert_no_endpoint_update(&mut endpoints_rx);
    } else {
        let paths = get_endpoint_paths(&mut endpoints_rx);
        assert!(paths.contains("services/supervisor"));
        assert!(paths.contains("services/counter"));
    }

    // Commit migration.
    let commit = CommitMigration::for_counter(&blockchain, 2);
    let tx = keypair.commit_migration(ToySupervisorService::INSTANCE_ID, commit);
    execute_transaction(&mut blockchain, tx).unwrap();
    assert_no_endpoint_update(&mut endpoints_rx);

    // Check that we're still accessing old service data.
    let snapshot = blockchain.snapshot();
    let schema: Schema<_> = snapshot
        .service_schema(CounterFactory::INSTANCE_ID)
        .unwrap();
    assert_eq!(schema.counter.get(), Some(1));
    // Check that transactions to the service are not dispatched.
    let tx = keypair.increment(CounterFactory::INSTANCE_ID, 5);
    drop(events.take());

    let err = execute_transaction(&mut blockchain, tx).unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::IncorrectInstanceId)
            .with_description_containing("unknown service with ID 100")
    );
    assert_no_endpoint_update(&mut endpoints_rx);

    // Check that the migrating service does not receive hooks.
    assert_eq!(
        events.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(5), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(5), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(6)),
        ]
    );

    // Flush migration. The service will transition to `Stopped` status.
    let tx = keypair.flush_migration(
        ToySupervisorService::INSTANCE_ID,
        CounterFactory::INSTANCE_NAME.to_owned(),
    );
    execute_transaction(&mut blockchain, tx).unwrap();
    let snapshot = blockchain.snapshot();
    let err = snapshot
        .service_schema::<Schema<_>, _>(CounterFactory::INSTANCE_ID)
        .unwrap_err();
    assert_matches!(err, ArtifactReqError::NoService);

    assert_eq!(
        events.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(6), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(6), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::CommitService(Height(7), old_spec, InstanceStatus::Stopped),
            RuntimeEvent::AfterCommit(Height(7)),
        ]
    );

    // Since service has transitioned from `Migrating` to `Stopped`, its endpoints should
    // be removed.
    let paths = get_endpoint_paths(&mut endpoints_rx);
    assert!(paths.contains("services/supervisor"));
    assert!(!paths.contains("services/counter"));
}

#[test]
fn basic_migration() {
    test_basic_migration(false);
}

#[test]
fn basic_migration_with_service_freeze() {
    test_basic_migration(true);
}

#[derive(Debug, Clone, Copy)]
struct RestartScenario {
    after_initiation: bool,
    after_commitment: bool,
    after_flush: bool,
}

fn check_state_after_restart(
    events: &EventsHandle,
    endpoints_rx: &mut mpsc::Receiver<UpdateEndpoints>,
) {
    let initial_events = events.take();
    assert_eq!(initial_events[0], RuntimeEvent::InitializeRuntime);
    assert_eq!(*initial_events.last().unwrap(), RuntimeEvent::ResumeRuntime);
    let supervisor = ToySupervisorService.default_instance().instance_spec;
    assert!(initial_events.iter().any(|event| match event {
        RuntimeEvent::CommitService(_, spec, InstanceStatus::Active) if *spec == supervisor => true,
        _ => false,
    }));

    let old_spec = CounterFactory::new(VERSIONS[0].parse().unwrap())
        .default_instance()
        .instance_spec;
    let counter_status = initial_events
        .iter()
        .filter_map(|event| match event {
            RuntimeEvent::CommitService(_, spec, status) if *spec == old_spec => Some(status),
            _ => None,
        })
        .next();
    let counter_status = counter_status.expect("No event regarding counter service");
    let is_migrating = match counter_status {
        InstanceStatus::Migrating(_) => true,
        InstanceStatus::Stopped => false,
        other => panic!("Unexpected counter status: {:?}", other),
    };

    // Check that endpoints of the migrating service are on.
    let paths = get_endpoint_paths(endpoints_rx);
    assert!(paths.contains("services/supervisor"));
    assert_eq!(paths.contains("services/counter"), is_migrating);
}

fn test_node_restart_during_migration(scenario: RestartScenario) {
    let db = Arc::new(TemporaryDB::new()) as Arc<dyn Database>;
    let (mut blockchain, ..) = create_runtime(Arc::clone(&db));
    let new_artifact = CounterFactory::new(VERSIONS[2].parse().unwrap()).artifact_id();
    let keypair = KeyPair::random();

    let tx = keypair.increment(CounterFactory::INSTANCE_ID, 1);
    execute_transaction(&mut blockchain, tx).unwrap();

    // Stop the service.
    let tx = keypair.stop_service(
        ToySupervisorService::INSTANCE_ID,
        CounterFactory::INSTANCE_ID,
    );
    execute_transaction(&mut blockchain, tx).unwrap();

    // Start async migration.
    let migration = MigrateService {
        instance_name: CounterFactory::INSTANCE_NAME.to_owned(),
        artifact: new_artifact,
    };
    let tx = keypair.migrate_service(ToySupervisorService::INSTANCE_ID, migration);
    execute_transaction(&mut blockchain, tx).unwrap();

    if scenario.after_initiation {
        let (new_blockchain, events, mut endpoints_rx) = create_runtime(Arc::clone(&db));
        blockchain = new_blockchain;
        check_state_after_restart(&events, &mut endpoints_rx);
    }

    // Commit migration.
    let commit = CommitMigration::for_counter(&blockchain, 2);
    let tx = keypair.commit_migration(ToySupervisorService::INSTANCE_ID, commit);
    execute_transaction(&mut blockchain, tx).unwrap();

    if scenario.after_commitment {
        let (new_blockchain, events, mut endpoints_rx) = create_runtime(Arc::clone(&db));
        blockchain = new_blockchain;
        check_state_after_restart(&events, &mut endpoints_rx);
    }

    let tx = keypair.flush_migration(
        ToySupervisorService::INSTANCE_ID,
        CounterFactory::INSTANCE_NAME.to_owned(),
    );
    execute_transaction(&mut blockchain, tx).unwrap();

    if scenario.after_flush {
        let (new_blockchain, events, mut endpoints_rx) = create_runtime(Arc::clone(&db));
        blockchain = new_blockchain;
        check_state_after_restart(&events, &mut endpoints_rx);
    }

    // Check that the service data has been updated.
    let snapshot = blockchain.snapshot();
    assert_eq!(
        snapshot.get_proof_entry::<_, u64>("counter.counter").get(),
        Some(2)
    );
}

#[test]
fn node_restart_after_migration_initiation() {
    test_node_restart_during_migration(RestartScenario {
        after_initiation: true,
        after_commitment: false,
        after_flush: false,
    });
}

#[test]
fn node_restart_after_migration_commitment() {
    test_node_restart_during_migration(RestartScenario {
        after_initiation: false,
        after_commitment: true,
        after_flush: false,
    });
}

#[test]
fn node_restart_after_migration_flush() {
    test_node_restart_during_migration(RestartScenario {
        after_initiation: false,
        after_commitment: false,
        after_flush: true,
    });
}

#[test]
fn node_restarts_after_each_migration_step() {
    test_node_restart_during_migration(RestartScenario {
        after_initiation: true,
        after_commitment: true,
        after_flush: true,
    });
}

fn perform_first_migration(blockchain: &mut BlockchainMut, new_artifact: ArtifactId) {
    let keypair = KeyPair::random();
    let tx = keypair.increment(CounterFactory::INSTANCE_ID, 1);
    execute_transaction(blockchain, tx).unwrap();

    // Freeze the service.
    let tx = keypair.freeze_service(
        ToySupervisorService::INSTANCE_ID,
        CounterFactory::INSTANCE_ID,
    );
    execute_transaction(blockchain, tx).unwrap();

    // Start async migration.
    let migration = MigrateService {
        instance_name: CounterFactory::INSTANCE_NAME.to_owned(),
        artifact: new_artifact,
    };
    let tx = keypair.migrate_service(ToySupervisorService::INSTANCE_ID, migration);
    execute_transaction(blockchain, tx).unwrap();

    // Commit migration.
    let commit = CommitMigration::for_counter(&blockchain, 2);
    let tx = keypair.commit_migration(ToySupervisorService::INSTANCE_ID, commit);
    execute_transaction(blockchain, tx).unwrap();

    // Flush migration.
    let tx = keypair.flush_migration(
        ToySupervisorService::INSTANCE_ID,
        CounterFactory::INSTANCE_NAME.to_owned(),
    );
    execute_transaction(blockchain, tx).unwrap();
}

#[test]
fn two_step_migration_without_intermediate_update() {
    let (mut blockchain, events, mut endpoints_rx) = create_runtime(TemporaryDB::new());
    let keypair = KeyPair::random();
    let new_artifact = CounterFactory::new(VERSIONS[2].parse().unwrap()).artifact_id();
    get_endpoint_paths(&mut endpoints_rx);

    perform_first_migration(&mut blockchain, new_artifact.clone());
    // Since service has transitioned from `Migrating` to `Stopped`, its endpoints should
    // be removed.
    let paths = get_endpoint_paths(&mut endpoints_rx);
    assert!(paths.contains("services/supervisor"));
    assert!(!paths.contains("services/counter"));

    // Start another async migration.
    let migration = MigrateService {
        instance_name: CounterFactory::INSTANCE_NAME.to_owned(),
        artifact: new_artifact.clone(),
    };
    let tx = keypair.migrate_service(ToySupervisorService::INSTANCE_ID, migration);
    drop(events.take());
    execute_transaction(&mut blockchain, tx).unwrap();
    let mut events_vec = events.take();
    let commit_service_event = events_vec.remove(3);

    assert_eq!(
        events_vec,
        vec![
            RuntimeEvent::BeforeTransactions(Height(6), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::MigrateService(new_artifact.clone(), Version::new(0, 1, 1)),
            RuntimeEvent::AfterTransactions(Height(6), ToySupervisorService::INSTANCE_ID),
            // The removed event would be here...
            RuntimeEvent::MigrateService(new_artifact, Version::new(0, 1, 1)),
            RuntimeEvent::AfterCommit(Height(7)),
        ]
    );
    let old_spec = CounterFactory::new(VERSIONS[0].parse().unwrap())
        .default_instance()
        .instance_spec;
    let migration = match commit_service_event {
        RuntimeEvent::CommitService(height, spec, InstanceStatus::Migrating(migration)) => {
            assert_eq!(height, Height(7));
            assert_eq!(spec, old_spec);
            migration
        }
        other => panic!("Unexpected event: {:?}", other),
    };
    assert_eq!(migration.end_version, Version::new(0, 1, 2));
    assert_eq!(migration.completed_hash, None);

    // Since the service is not associated with a deployed artifact, service endpoints
    // should remain switched off.
    assert_no_endpoint_update(&mut endpoints_rx);
}

#[test]
fn two_step_migration_with_intermediate_update() {
    let (mut blockchain, events, mut endpoints_rx) = create_runtime(TemporaryDB::new());
    let keypair = KeyPair::random();
    let new_artifact = CounterFactory::new(VERSIONS[2].parse().unwrap()).artifact_id();
    get_endpoint_paths(&mut endpoints_rx);

    perform_first_migration(&mut blockchain, new_artifact.clone());
    get_endpoint_paths(&mut endpoints_rx); // endpoint removal, as in the previous example.

    // Fast-forward the service to the intermediate artifact.
    let intermediate_factory = CounterFactory::new(Version::new(0, 1, 1));
    let intermediate_artifact = intermediate_factory.artifact_id();
    let intermediate_spec = intermediate_factory.default_instance().instance_spec;
    let migration = MigrateService {
        instance_name: CounterFactory::INSTANCE_NAME.to_owned(),
        artifact: intermediate_artifact.clone(),
    };
    let tx = keypair.migrate_service(ToySupervisorService::INSTANCE_ID, migration);

    drop(events.take());
    execute_transaction(&mut blockchain, tx).unwrap();
    assert_eq!(
        events.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(6), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::MigrateService(intermediate_artifact, Version::new(0, 1, 1)),
            RuntimeEvent::AfterTransactions(Height(6), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::CommitService(Height(7), intermediate_spec, InstanceStatus::Stopped),
            RuntimeEvent::AfterCommit(Height(7)),
        ]
    );

    // Start another async migration.
    let migration = MigrateService {
        instance_name: CounterFactory::INSTANCE_NAME.to_owned(),
        artifact: new_artifact,
    };
    let tx = keypair.migrate_service(ToySupervisorService::INSTANCE_ID, migration);
    execute_transaction(&mut blockchain, tx).unwrap();

    // Since the service has been associated with a deployed artifact, service endpoints
    // should remain switched on.
    let paths = get_endpoint_paths(&mut endpoints_rx);
    assert!(paths.contains("services/supervisor"));
    assert!(paths.contains("services/counter"));
}

#[test]
fn resume_with_incorrect_artifact_version() {
    let (mut blockchain, ..) = create_runtime(TemporaryDB::new());
    let keypair = KeyPair::random();
    let new_artifact = CounterFactory::new(VERSIONS[2].parse().unwrap()).artifact_id();

    perform_first_migration(&mut blockchain, new_artifact);

    let resume = ResumeService {
        instance_id: CounterFactory::INSTANCE_ID,
        params: vec![],
    };
    let tx = keypair.resume_service(ToySupervisorService::INSTANCE_ID, resume);
    let actual_err = execute_transaction(&mut blockchain, tx).unwrap_err();
    let expected_msg = "Cannot resume service `100:counter` because its data version (0.1.1) \
                        does not match the associated artifact `0:counter:0.1.0`";
    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&CoreError::CannotResumeService)
            .with_description_containing(expected_msg)
    );

    // Check that the problem is solved by fast-forward migration as the error description suggests.
    let intermediate_factory = CounterFactory::new(Version::new(0, 1, 1));
    let intermediate_artifact = intermediate_factory.artifact_id();
    let migration = MigrateService {
        instance_name: CounterFactory::INSTANCE_NAME.to_owned(),
        artifact: intermediate_artifact,
    };
    let tx = keypair.migrate_service(ToySupervisorService::INSTANCE_ID, migration);
    execute_transaction(&mut blockchain, tx).unwrap();

    let resume = ResumeService {
        instance_id: CounterFactory::INSTANCE_ID,
        params: vec![0], // get different transaction hash
    };
    let tx = keypair.resume_service(ToySupervisorService::INSTANCE_ID, resume);
    execute_transaction(&mut blockchain, tx).unwrap();

    // Check that the service processes transactions.
    let tx = keypair.increment(CounterFactory::INSTANCE_ID, 5);
    execute_transaction(&mut blockchain, tx).unwrap();
}
