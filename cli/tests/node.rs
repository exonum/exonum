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
use exonum_rust_runtime::{
    api::ServiceApiBuilder, spec::Spec, DefaultInstance, Service, ServiceFactory,
};
use exonum_supervisor::api::DispatcherInfo;
use lazy_static::lazy_static;
use reqwest::RequestBuilder;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use tokio::time::delay_for;

use std::{
    net::{Ipv4Addr, SocketAddr, TcpListener},
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
            .endpoint("answer", |_state, _query: ()| async { Ok(42) });
    }
}

impl DefaultInstance for SimpleService {
    const INSTANCE_ID: u32 = 100;
    const INSTANCE_NAME: &'static str = "simple";
}

async fn send_request<T>(request: RequestBuilder) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    request
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .map_err(From::from)
}

#[tokio::test]
async fn node_basic_workflow() -> anyhow::Result<()> {
    let public_addr = PUBLIC_ADDRS[0];
    let public_api_root = format!("http://{}/api", public_addr);
    let public_addr = public_addr.to_string();
    let private_addr = PRIVATE_ADDRS[0].to_string();

    let dir = TempDir::new()?;
    let dir_path = dir.path().as_os_str();
    let args = vec![
        "run-dev".as_ref(),
        "--blockchain-path".as_ref(),
        dir_path,
        "--public-api-address".as_ref(),
        public_addr.as_ref(),
        "--private-api-address".as_ref(),
        private_addr.as_ref(),
    ];

    let node = NodeBuilder::with_args(args)
        .with(
            Spec::new(SimpleService)
                .with_default_instance()
                .with_instance(200, "other", ()),
        )
        .execute_command()?
        .unwrap();
    let shutdown_handle = node.shutdown_handle();
    let node_task = tokio::spawn(node.run());
    delay_for(Duration::from_secs(2)).await;

    let client = reqwest::Client::new();
    // Check info about deployed artifacts returned via supervisor API.
    let url = format!("{}/services/supervisor/services", public_api_root);
    let info: DispatcherInfo = send_request(client.get(&url)).await?;

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
    let url = format!(
        "{}/explorer/v1/blocks?count=1&add_precommits=true",
        public_api_root
    );
    loop {
        let BlocksRange { blocks, .. } = send_request(client.get(&url)).await?;
        assert_eq!(blocks.len(), 1);
        if blocks[0].block.height > Height(0) {
            assert_eq!(blocks[0].precommits.as_ref().unwrap().len(), 1);
            break;
        }
        delay_for(Duration::from_millis(200)).await;
    }

    // Check API of two started service instances.
    let url = format!("{}/services/simple/answer", public_api_root);
    let answer: u64 = send_request(client.get(&url)).await?;
    assert_eq!(answer, 42);
    let url = format!("{}/services/other/answer", public_api_root);
    let answer: u64 = send_request(client.get(&url)).await?;
    assert_eq!(answer, 42);

    shutdown_handle.shutdown().await?;
    node_task.await??;
    Ok(())
}
