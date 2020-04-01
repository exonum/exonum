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

// cspell:ignore unistd

#![cfg(unix)]

use futures::StreamExt;
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use rusty_fork::{fork, rusty_fork_id, ChildWrapper};
use tokio::{
    runtime::Runtime,
    signal::unix::{signal, SignalKind},
};

use std::{
    env,
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    thread,
    time::Duration,
};

pub mod common;
use crate::common::{run_nodes, Options};

fn check_child_start(child: &mut ChildWrapper) {
    let maybe_status = child
        .wait_timeout(Duration::from_secs(5))
        .expect("Failed to wait for node to function");
    if let Some(status) = maybe_status {
        panic!(
            "Node exited unexpectedly with this exit status: {:?}",
            status
        );
    }
}

fn check_child_exit(child: &mut ChildWrapper, output: &mut File) {
    // Check that the child has exited.
    let exit_status = child
        .wait_timeout(Duration::from_secs(2))
        .expect("Failed to wait for node exit")
        .unwrap_or_else(|| {
            child.kill().ok();
            panic!("Node did not exit in 2 secs after being sent the signal");
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

fn check_child(child: &mut ChildWrapper, output: &mut File, signal: Signal) {
    // Sleep several seconds in order for the node to launch.
    check_child_start(child);

    // Send a signal to the node.
    let pid = Pid::from_raw(child.id() as i32);
    kill(pid, signal).unwrap();

    // Check that the child has exited.
    check_child_exit(child, output);
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

fn check_child_with_custom_handler(
    child: &mut ChildWrapper,
    output: &mut File,
    http_port: Option<u16>,
) {
    // Sleep several seconds in order for the node to launch.
    check_child_start(child);

    // Send a signal to the node.
    let pid = Pid::from_raw(child.id() as i32);
    for _ in 0..2 {
        kill(pid, Signal::SIGTERM).unwrap();
        thread::sleep(Duration::from_secs(1));

        // Check that the node did not exit due to a custom handler.
        let maybe_status = child.try_wait().expect("Failed to query child exit status");
        if let Some(status) = maybe_status {
            panic!(
                "Node exited unexpectedly with this exit status: {:?}",
                status
            );
        }
    }

    if let Some(http_port) = http_port {
        // Check that the HTTP server is still functional by querying the Rust runtime.
        let url = format!(
            "http://127.0.0.1:{}/api/runtimes/rust/proto-sources?type=core",
            http_port
        );
        let response = reqwest::blocking::get(&url).unwrap();
        assert!(response.status().is_success(), "{:?}", response);
    }

    // Send the third signal. The node should now exit.
    kill(pid, Signal::SIGTERM).unwrap();
    check_child_exit(child, output);
}

async fn start_node_without_signals(start_port: u16, with_http: bool) {
    let mut options = Options::default();
    options.disable_signals = true;
    if with_http {
        options.http_start_port = Some(start_port + 1);
    }

    let (mut nodes, _) = run_nodes(1, start_port, options);
    let node = nodes.pop().unwrap();

    // Register a SIGTERM handler that terminates the node after several signals are received.
    let mut signal = signal(SignalKind::terminate()).unwrap().skip(2);
    let shutdown_handle = node.shutdown_handle();
    tokio::spawn(async move {
        signal.next().await;
        println!("Shutting down node handler");
        shutdown_handle.shutdown().await.unwrap();
    });

    node.run().await
}

#[test]
fn interrupt_node_without_http() {
    fork(
        "interrupt_node_without_http",
        rusty_fork_id!(),
        |_| {},
        |child, output| check_child(child, output, Signal::SIGINT),
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
        |child, output| check_child(child, output, Signal::SIGINT),
        || {
            let mut runtime = Runtime::new().unwrap();
            runtime.block_on(start_node(16_460, true));
        },
    )
    .unwrap();
}

#[test]
fn terminate_node_without_http() {
    fork(
        "terminate_node_without_http",
        rusty_fork_id!(),
        |_| {},
        |child, output| check_child(child, output, Signal::SIGTERM),
        || {
            let mut runtime = Runtime::new().unwrap();
            runtime.block_on(start_node(16_470, false));
        },
    )
    .unwrap();
}

#[test]
fn terminate_node_with_http() {
    fork(
        "terminate_node_with_http",
        rusty_fork_id!(),
        |_| {},
        |child, output| check_child(child, output, Signal::SIGTERM),
        || {
            let mut runtime = Runtime::new().unwrap();
            runtime.block_on(start_node(16_480, true));
        },
    )
    .unwrap();
}

#[test]
fn quit_node_without_http() {
    fork(
        "quit_node_without_http",
        rusty_fork_id!(),
        |_| {},
        |child, output| check_child(child, output, Signal::SIGQUIT),
        || {
            let mut runtime = Runtime::new().unwrap();
            runtime.block_on(start_node(16_490, false));
        },
    )
    .unwrap();
}

#[test]
fn quit_node_with_http() {
    fork(
        "quit_node_with_http",
        rusty_fork_id!(),
        |_| {},
        |child, output| check_child(child, output, Signal::SIGQUIT),
        || {
            let mut runtime = Runtime::new().unwrap();
            runtime.block_on(start_node(16_500, true));
        },
    )
    .unwrap();
}

#[test]
fn term_node_with_custom_handling_and_http() {
    fork(
        "term_node_with_custom_handling_and_http",
        rusty_fork_id!(),
        |_| {},
        |child, output| check_child_with_custom_handler(child, output, Some(16_511)),
        || {
            let mut runtime = Runtime::new().unwrap();
            runtime.block_on(start_node_without_signals(16_510, true));
        },
    )
    .unwrap();
}

#[test]
fn term_node_with_custom_handling_and_no_http() {
    fork(
        "term_node_with_custom_handling_and_no_http",
        rusty_fork_id!(),
        |_| {},
        |child, output| check_child_with_custom_handler(child, output, None),
        || {
            let mut runtime = Runtime::new().unwrap();
            runtime.block_on(start_node_without_signals(16_520, true));
        },
    )
    .unwrap();
}
