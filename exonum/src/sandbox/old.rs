// Copyright 2018 The Exonum Team
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

use std::time::Duration;

use super::{
    sandbox::timestamping_sandbox, sandbox_tests_helper::{gen_timestamping_tx, NOT_LOCKED},
};
use blockchain::Block;
use crypto::{CryptoHash, Hash};
use helpers::{Height, Round, ValidatorId};

#[test]
fn test_send_propose_and_prevote() {
    let sandbox = timestamping_sandbox();

    // get some tx
    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    // round happens
    sandbox.add_time(Duration::from_millis(1000));
    sandbox.add_time(Duration::from_millis(1999));

    sandbox.assert_state(Height(1), Round(3));

    // ok, we are leader
    let propose = sandbox.create_propose(
        ValidatorId(0),
        Height(1),
        Round(3),
        &sandbox.last_hash(),
        &[tx.hash()],
        sandbox.s(ValidatorId(0)),
    );

    sandbox.broadcast(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(3),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(0)),
    ));
}

#[test]
fn test_send_prevote() {
    let sandbox = timestamping_sandbox();

    let propose = sandbox.create_propose(
        ValidatorId(2),
        Height(1),
        Round(1),
        &sandbox.last_hash(),
        &[],
        sandbox.s(ValidatorId(2)),
    );

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(0)),
    ));
}

#[test]
fn test_get_lock_and_send_precommit() {
    let sandbox = timestamping_sandbox();

    let propose = sandbox.create_propose(
        ValidatorId(2),
        Height(1),
        Round(1),
        &sandbox.last_hash(),
        &[],
        sandbox.s(ValidatorId(2)),
    );

    let block = Block::new(
        ValidatorId(2),
        Height(1),
        0,
        &sandbox.last_hash(),
        &Hash::zero(),
        &sandbox.last_state_hash(),
    );

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(0)),
    ));
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None);
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(2)),
    ));
    sandbox.broadcast(&sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(0)),
    ));
    sandbox.assert_lock(Round(1), Some(propose.hash()));
}

#[test]
fn test_commit() {
    let sandbox = timestamping_sandbox();

    let propose = sandbox.create_propose(
        ValidatorId(2),
        Height(1),
        Round(1),
        &sandbox.last_hash(),
        &[],
        sandbox.s(ValidatorId(2)),
    );

    let block = Block::new(
        ValidatorId(2),
        Height(1),
        0,
        &sandbox.last_hash(),
        &Hash::zero(),
        &sandbox.last_state_hash(),
    );

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(0)),
    ));
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(1)),
    ));
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(2)),
    ));
    sandbox.broadcast(&sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(0)),
    ));
    sandbox.recv(&sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &propose.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(2)),
    ));
    sandbox.recv(&sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        &propose.hash(),
        &propose.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(3)),
    ));
    sandbox.assert_state(Height(1), Round(1));
}

#[test]
#[should_panic(expected = "Expected to broadcast the message Consensus(Prevote")]
fn received_unexpected_propose() {
    let sandbox = timestamping_sandbox();

    let propose = sandbox.create_propose(
        ValidatorId(1),
        Height::zero(),
        Round(1),
        &sandbox.last_hash(),
        &[],
        sandbox.s(ValidatorId(1)),
    );

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height::zero(),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(0)),
    ));
}
