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

//! Tests node creation with the help of the `run-dev` command.

use exonum::{
    helpers::Height,
    runtime::{versioning::Version, InstanceStatus, SUPERVISOR_INSTANCE_ID},
};
use exonum_derive::*;
use exonum_explorer_service::api::BlocksRange;
use exonum_rust_runtime::{api::ServiceApiBuilder, DefaultInstance, Service, ServiceFactory};
use exonum_system_api::public::DispatcherInfo;
use futures::Future;
use lazy_static::lazy_static;
use tempfile::TempDir;

use std::{
    net::{Ipv4Addr, SocketAddr, TcpListener},
    thread,
    time::Duration,
};

use exonum_cli::NodeBuilder;

const PORTS: usize = 1;
lazy_static! {
    static ref PUBLIC_ADDRS: Vec<SocketAddr> = unused_addresses(8_000, PORTS);
    static ref PRIVATE_ADDRS: Vec<SocketAddr> = unused_addresses(9_000, PORTS);
}

fn unused_addresses(start: u16, count: usize) -> Vec<SocketAddr> {
    let listeners: Vec<_> = (start..)
        .filter_map(|port| TcpListener::bind((Ipv4Addr::LOCALHOST, port)).ok())
        .take(count)
        .collect();
    listeners
        .into_iter()
        .map(|listener| listener.local_addr().unwrap())
        .collect()
}

#[derive(Debug, Clone, Copy, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "simple-service", artifact_version = "0.1.0")]
struct SimpleService;

impl Service for SimpleService {
    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        builder
            .public_scope()
            .endpoint("answer", |_state, _query: ()| Ok(42));
    }
}

impl DefaultInstance for SimpleService {
    const INSTANCE_ID: u32 = 100;
    const INSTANCE_NAME: &'static str = "simple";
}

#[test]
fn node_basic_workflow() -> Result<(), failure::Error> {
    let public_addr = PUBLIC_ADDRS[0];
    let private_addr = PRIVATE_ADDRS[0];
    let public_api_root = format!("http://{}/api", public_addr);

    let dir = TempDir::new()?;
    let dir_path = dir
        .path()
        .to_str()
        .expect("Path to temporary directory cannot be encoded in UTF-8 string");
    let args = vec![
        "run-dev".to_owned(),
        "-a".to_owned(),
        dir_path.to_owned(),
        "--public-api-address".to_owned(),
        public_addr.to_string(),
        "--private-api-address".to_owned(),
        private_addr.to_string(),
    ];

    let other_instance = SimpleService
        .artifact_id()
        .into_default_instance(200, "other");
    let node = NodeBuilder::with_args(args)
        .with_default_rust_service(SimpleService)
        .with_instance(other_instance)
        .execute_command()?
        .unwrap();
    let shutdown_handle = node.shutdown_handle();
    let node_thread = thread::spawn(|| {
        node.run().ok();
    });
    thread::sleep(Duration::from_secs(2));

    let client = reqwest::Client::new();
    // Check info returned by the system API plugin.
    let info: DispatcherInfo = client
        .get(&format!("{}/system/v1/services", public_api_root))
        .send()?
        .error_for_status()?
        .json()?;

    let simple_service_artifact = SimpleService.artifact_id();
    assert!(info.artifacts.contains(&simple_service_artifact));
    let has_simple_service = info.services.iter().any(|instance_state| {
        let spec = &instance_state.spec;
        spec.id == SimpleService::INSTANCE_ID
            && spec.name == SimpleService::INSTANCE_NAME
            && spec.artifact.name == "simple-service"
            && spec.artifact.version == Version::new(0, 1, 0)
            && instance_state.status == Some(InstanceStatus::Active)
    });
    assert!(has_simple_service);

    let has_supervisor = info.services.iter().any(|instance_state| {
        let spec = &instance_state.spec;
        spec.id == SUPERVISOR_INSTANCE_ID && spec.name == "supervisor"
    });
    assert!(has_supervisor);

    // Check explorer API.
    loop {
        let BlocksRange { blocks, .. } = client
            .get(&format!(
                "{}/explorer/v1/blocks?count=1&add_precommits=true",
                public_api_root
            ))
            .send()?
            .error_for_status()?
            .json()?;
        assert_eq!(blocks.len(), 1);
        if blocks[0].block.height > Height(0) {
            assert_eq!(blocks[0].precommits.as_ref().unwrap().len(), 1);
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }

    // Check API of two started service instances.
    let answer: u64 = client
        .get(&format!("{}/services/simple/answer", public_api_root))
        .send()?
        .error_for_status()?
        .json()?;
    assert_eq!(answer, 42);
    let answer: u64 = client
        .get(&format!("{}/services/other/answer", public_api_root))
        .send()?
        .error_for_status()?
        .json()?;
    assert_eq!(answer, 42);

    shutdown_handle.shutdown().wait()?;
    node_thread.join().ok();
    Ok(())
}
