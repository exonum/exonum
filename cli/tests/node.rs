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

//! Tests node creation.

use exonum::{
    helpers::Height,
    runtime::{versioning::Version, InstanceStatus},
};
use exonum_derive::*;
use exonum_explorer_service::api::BlocksRange;
use exonum_rust_runtime::{api::ServiceApiBuilder, DefaultInstance, Service, ServiceFactory};
use exonum_system_api::public::DispatcherInfo;
use futures::Future;
use tempfile::TempDir;

use std::{thread, time::Duration};

use exonum_cli::NodeBuilder;

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
    let dir = TempDir::new()?;
    let dir_path = dir
        .path()
        .to_str()
        .expect("Path to temporary directory cannot be encoded in UTF-8 string");
    let args = vec![
        "node-executable".to_owned(),
        "run-dev".to_owned(),
        "-a".to_owned(),
        dir_path.to_owned(),
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
    thread::sleep(Duration::from_secs(5));

    let client = reqwest::Client::new();
    // Check info returned by the system API plugin.
    let info: DispatcherInfo = client
        .get("http://127.0.0.1:8080/api/system/v1/services")
        .send()?
        .error_for_status()?
        .json()?;

    let simple_service_artifact = SimpleService.artifact_id();
    assert!(info.artifacts.contains(&simple_service_artifact));
    assert!(info.services.iter().any(|instance_state| {
        let spec = &instance_state.spec;
        let is_simple = spec.id == SimpleService::INSTANCE_ID
            && spec.name == SimpleService::INSTANCE_NAME
            && spec.artifact.name == "simple-service"
            && spec.artifact.version == Version::new(0, 1, 0)
            && instance_state.status == Some(InstanceStatus::Active);
        is_simple
    }));

    // Check explorer API.
    let BlocksRange { blocks, .. } = client
        .get("http://127.0.0.1:8080/api/explorer/v1/blocks?count=1")
        .send()?
        .error_for_status()?
        .json()?;
    assert_eq!(blocks.len(), 1);
    assert!(blocks[0].block.height > Height(0));

    // Check service API.
    let answer: u64 = client
        .get("http://127.0.0.1:8080/api/services/simple/answer")
        .send()?
        .error_for_status()?
        .json()?;
    assert_eq!(answer, 42);

    let answer: u64 = client
        .get("http://127.0.0.1:8080/api/services/other/answer")
        .send()?
        .error_for_status()?
        .json()?;
    assert_eq!(answer, 42);

    shutdown_handle.shutdown().wait().unwrap();
    node_thread.join().ok();
    Ok(())
}
