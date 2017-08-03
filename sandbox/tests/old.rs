// Copyright 2017 The Exonum Team
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

extern crate exonum;
extern crate sandbox;

use std::time::Duration;

use exonum::messages::{Message, Propose, Prevote, Precommit};
use exonum::blockchain::{BlockHeader, SCHEMA_MAJOR_VERSION};
use exonum::crypto::Hash;

use sandbox::timestamping_sandbox;
use sandbox::sandbox_tests_helper::gen_timestamping_tx;

#[test]
fn test_send_propose_and_prevote() {
    let sandbox = timestamping_sandbox();

    // get some tx
    let tx = gen_timestamping_tx();
    sandbox.recv(tx.clone());

    // round happens
    sandbox.add_time(Duration::from_millis(1000));
    sandbox.add_time(Duration::from_millis(1999));

    sandbox.assert_state(1, 3);

    // ok, we are leader
    let propose = Propose::new(0, 1, 3, &sandbox.last_hash(), &[tx.hash()], sandbox.s(0));

    sandbox.broadcast(propose.clone());
    sandbox.broadcast(Prevote::new(0, 1, 3, &propose.hash(), 0, sandbox.s(0)));
}

#[test]
fn test_send_prevote() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(2, 1, 1, &sandbox.last_hash(), &[], sandbox.s(2));

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 1, 1, &propose.hash(), 0, sandbox.s(0)));
}

#[test]
fn test_get_lock_and_send_precommit() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(2, 1, 1, &sandbox.last_hash(), &[], sandbox.s(2));

    let block = BlockHeader::new(
        SCHEMA_MAJOR_VERSION,
        2,
        1,
        0,
        &sandbox.last_hash(),
        &Hash::zero(),
        &sandbox.last_state_hash(),
    );

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 1, 1, &propose.hash(), 0, sandbox.s(0)));
    sandbox.recv(Prevote::new(1, 1, 1, &propose.hash(), 0, sandbox.s(1)));
    sandbox.assert_lock(0, None);
    sandbox.recv(Prevote::new(2, 1, 1, &propose.hash(), 0, sandbox.s(2)));
    sandbox.broadcast(Precommit::new(
        0,
        1,
        1,
        &propose.hash(),
        &block.hash(),
        sandbox.time(),
        sandbox.s(0),
    ));
    sandbox.assert_lock(1, Some(propose.hash()));
}

#[test]
fn test_commit() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(2, 1, 1, &sandbox.last_hash(), &[], sandbox.s(2));

    let block = BlockHeader::new(
        SCHEMA_MAJOR_VERSION,
        2,
        1,
        0,
        &sandbox.last_hash(),
        &Hash::zero(),
        &sandbox.last_state_hash(),
    );

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 1, 1, &propose.hash(), 0, sandbox.s(0)));
    sandbox.recv(Prevote::new(1, 1, 1, &propose.hash(), 0, sandbox.s(1)));
    sandbox.recv(Prevote::new(2, 1, 1, &propose.hash(), 0, sandbox.s(2)));
    sandbox.broadcast(Precommit::new(
        0,
        1,
        1,
        &propose.hash(),
        &block.hash(),
        sandbox.time(),
        sandbox.s(0),
    ));
    sandbox.recv(Precommit::new(
        2,
        1,
        1,
        &propose.hash(),
        &propose.hash(),
        sandbox.time(),
        sandbox.s(2),
    ));
    sandbox.recv(Precommit::new(
        3,
        1,
        1,
        &propose.hash(),
        &propose.hash(),
        sandbox.time(),
        sandbox.s(3),
    ));
    sandbox.assert_state(1, 1);
}

// TODO: Remove `#[rustfmt_skip]` after https://github.com/rust-lang-nursery/rustfmt/issues/1777
// is fixed.
#[cfg_attr(rustfmt, rustfmt_skip)]
#[test]
#[should_panic(expected = "Expected to broadcast the message Consensus(Prevote")]
fn received_unexpected_propose() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(1, 0, 1, &sandbox.last_hash(), &[], sandbox.s(1));

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 0, 1, &propose.hash(), 0, sandbox.s(0)));
}
