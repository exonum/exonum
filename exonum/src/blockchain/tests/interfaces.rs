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

use exonum_merkledb::Snapshot;
use futures::{Future, IntoFuture};
use pretty_assertions::assert_eq;
use semver::Version;

use crate::{
    blockchain::{
        config::{GenesisConfigBuilder, InstanceInitParams},
        Blockchain, BlockchainBuilder, BlockchainMut,
    },
    runtime::{
        migrations::{InitMigrationError, MigrationScript},
        ArtifactId, CallInfo, DispatcherSchema, ExecutionContext, ExecutionError, InstanceId,
        InstanceSpec, InstanceStatus, Mailbox, Runtime, WellKnownRuntime,
    },
};

use super::create_consensus_config;

/// Interfaces runtime is a runtime with hard-coded three services that
/// are already deployed and initialized.
/// There are no actual "services", as we don't test any TX logic here,
/// but for any of those three services there is a set of interfaces implemented by them.
#[derive(Debug, Default)]
struct InterfacesRuntime;

impl InterfacesRuntime {
    /// Creates a dummy artifact for services in `InterfacesRuntime`.
    pub fn default_artifact() -> ArtifactId {
        ArtifactId::new(Self::ID, "interfaces-artifact", Version::new(1, 0, 0)).unwrap()
    }

    pub fn instance(id: InstanceId) -> InstanceInitParams {
        InstanceInitParams::new(
            id,
            format!("service-{}", id),
            Self::default_artifact(),
            vec![],
        )
    }

    pub fn default_instances() -> Vec<InstanceInitParams> {
        vec![
            Self::instance(FIRST_SERVICE_ID),
            Self::instance(SECOND_SERVICE_ID),
            Self::instance(THIRD_SERVICE_ID),
        ]
    }
}

impl WellKnownRuntime for InterfacesRuntime {
    const ID: u32 = 254;
}

const FIRST_SERVICE_ID: InstanceId = 10;
const FIRST_SERVICE_INTERFACES: [&'static str; 3] = ["", "interface_a", "interface_b"];
const SECOND_SERVICE_ID: InstanceId = 11;
const SECOND_SERVICE_INTERFACES: [&'static str; 2] = ["", "interface_a"];
const THIRD_SERVICE_ID: InstanceId = 12;
const THIRD_SERVICE_INTERFACES: [&'static str; 2] = ["", "interface_b"];

impl Runtime for InterfacesRuntime {
    fn deploy_artifact(
        &mut self,
        _artifact: ArtifactId,
        _deploy_spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        let res = Ok(());
        Box::new(res.into_future())
    }

    fn is_artifact_deployed(&self, _id: &ArtifactId) -> bool {
        true
    }

    fn interfaces(&self, id: InstanceId) -> Vec<String> {
        let interfaces = match id {
            FIRST_SERVICE_ID => FIRST_SERVICE_INTERFACES.as_ref(),
            SECOND_SERVICE_ID => SECOND_SERVICE_INTERFACES.as_ref(),
            THIRD_SERVICE_ID => THIRD_SERVICE_INTERFACES.as_ref(),
            _ => panic!("There are only three services in InterfacesRuntime"),
        };

        interfaces.iter().map(ToString::to_string).collect()
    }

    fn initiate_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _spec: &InstanceSpec,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn update_service_status(
        &mut self,
        _snapshot: &dyn Snapshot,
        _spec: &InstanceSpec,
        _status: &InstanceStatus,
    ) {
    }

    fn migrate(
        &self,
        _new_artifact: &ArtifactId,
        _data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        Err(InitMigrationError::NotSupported)
    }

    fn execute(
        &self,
        _context: ExecutionContext<'_>,
        _call_info: &CallInfo,
        _arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        Ok(())
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
        _context: ExecutionContext<'_>,
        _instance_id: InstanceId,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {}
}

/// Creates a blockchain, panicking if it can't be done.
fn create_blockchain() -> BlockchainMut {
    let genesis_config = InterfacesRuntime::default_instances()
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

    let runtime = InterfacesRuntime::default();

    BlockchainBuilder::new(Blockchain::build_for_tests(), genesis_config)
        .with_runtime(runtime)
        .build()
}

/// Checks that `DispatcherSchema` stores information about interfaces
/// implemented by service instances.
#[test]
fn stored_interfaces() {
    let blockchain = create_blockchain();

    let snapshot = blockchain.snapshot();

    let schema = DispatcherSchema::new(&snapshot);

    let test_vector = [
        (FIRST_SERVICE_ID, FIRST_SERVICE_INTERFACES.as_ref()),
        (SECOND_SERVICE_ID, SECOND_SERVICE_INTERFACES.as_ref()),
        (THIRD_SERVICE_ID, THIRD_SERVICE_INTERFACES.as_ref()),
    ];
    for (id, interfaces) in &test_vector {
        let test_service_interfaces = schema
            .service_interfaces()
            .get(&id)
            .expect("Interfaces for test service aren't stored");

        let expected_interfaces = interfaces.iter().map(ToString::to_string).collect();

        assert_eq!(test_service_interfaces.inner, expected_interfaces);
    }
}

/// Checks that `DispatcherSchema::get_instances_by_interface` returns all services
/// that implement certain interface.
#[test]
fn dispatcher_get_instances_by_interface() {
    let blockchain = create_blockchain();

    let snapshot = blockchain.snapshot();

    let schema = DispatcherSchema::new(&snapshot);

    let test_vector = [
        (
            "",
            vec![FIRST_SERVICE_ID, SECOND_SERVICE_ID, THIRD_SERVICE_ID],
        ),
        ("interface_a", vec![FIRST_SERVICE_ID, SECOND_SERVICE_ID]),
        ("interface_b", vec![FIRST_SERVICE_ID, THIRD_SERVICE_ID]),
        ("nonexistent_interface", vec![]),
    ];

    for (interface, expected_ids) in &test_vector {
        let service_ids = schema.get_instances_by_interface(interface);

        assert_eq!(service_ids, *expected_ids);
    }
}
