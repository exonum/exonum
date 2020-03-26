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

//! Tests related to signal handling by the nodes.

#![cfg(unix)]

use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use rusty_fork::{fork, rusty_fork_id, ChildWrapper};
use tokio::runtime::Runtime;

use std::{
    env,
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    time::Duration,
};

pub mod common;
use crate::common::{run_nodes, Options};

fn check_child(child: &mut ChildWrapper, output: &mut File) {
    // Sleep several seconds in order for the node to launch.
    let maybe_status = child
        .wait_timeout(Duration::from_secs(5))
        .expect("Failed to wait for node to function");
    if let Some(status) = maybe_status {
        panic!(
            "Node exited unexpectedly with this exit status: {:?}",
            status
        );
    }

    // Send a SIGINT to the node.
    let pid = Pid::from_raw(child.id() as i32);
    kill(pid, Signal::SIGINT).unwrap();

    // Check that the child has exited.
    let exit_status = child
        .wait_timeout(Duration::from_secs(2))
        .expect("Failed to wait for node exit")
        .unwrap_or_else(|| {
            child.kill().ok();
            panic!("Node did not exit in 2 secs after being sent SIGINT");
        });

    assert!(
        exit_status.success(),
        "Node exited with unexpected status: {:?}",
        exit_status
    );

    output.seek(SeekFrom::Start(0)).unwrap();
    let reader = BufReader::new(&*output);
    for line_res in reader.lines() {
        if let Ok(line) = line_res {
            if line.contains("Shutting down node handler") {
                return;
            }
        }
    }
    panic!("Node did not shut down properly");
}

async fn start_node(start_port: u16, with_http: bool) {
    // Enable logs in order to check that the node shuts down properly.
    env::set_var("RUST_LOG", "exonum_node=info");
    exonum::helpers::init_logger().ok();

    let mut options = Options::default();
    if with_http {
        options.http_start_port = Some(start_port + 1);
    }

    let (mut nodes, _) = run_nodes(1, start_port, options);
    let node = nodes.pop().unwrap();
    node.run().await
}

#[test]
fn interrupt_node_without_http() {
    fork(
        "interrupt_node_without_http",
        rusty_fork_id!(),
        |_| {},
        check_child,
        || {
            let mut runtime = Runtime::new().unwrap();
            runtime.block_on(start_node(16_450, false));
        },
    )
    .unwrap();
}

#[test]
fn interrupt_node_with_http() {
    fork(
        "interrupt_node_with_http",
        rusty_fork_id!(),
        |_| {},
        check_child,
        || {
            let mut runtime = Runtime::new().unwrap();
            runtime.block_on(start_node(16_460, true));
        },
    )
    .unwrap();
}
