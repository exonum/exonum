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
    sandbox::timestamping_sandbox,
    sandbox_tests_helper::{
        gen_timestamping_tx, VALIDATOR_0, VALIDATOR_1, VALIDATOR_2, VALIDATOR_3, HEIGHT_ONE,
        ROUND_ONE, ROUND_THREE,
    },
};
use blockchain::{Block, SCHEMA_MAJOR_VERSION};
use crypto::{CryptoHash, Hash};
use helpers::{Height, Round};
use messages::{Precommit, Prevote, Propose};

#[test]
fn test_send_propose_and_prevote() {
    let sandbox = timestamping_sandbox();

    // get some tx
    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    // round happens
    sandbox.add_time(Duration::from_millis(1000));
    sandbox.add_time(Duration::from_millis(1999));

    sandbox.assert_state(HEIGHT_ONE, ROUND_THREE);

    // ok, we are leader
    let propose = Propose::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_THREE,
        &sandbox.last_hash(),
        &[tx.hash()],
        sandbox.s(VALIDATOR_0),
    );

    sandbox.broadcast(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_0),
    ));
}

#[test]
fn test_send_prevote() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &sandbox.last_hash(),
        &[],
        sandbox.s(VALIDATOR_2),
    );

    sandbox.recv(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_0),
    ));
}

#[test]
fn test_get_lock_and_send_precommit() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &sandbox.last_hash(),
        &[],
        sandbox.s(VALIDATOR_2),
    );

    let block = Block::new(
        SCHEMA_MAJOR_VERSION,
        VALIDATOR_2,
        HEIGHT_ONE,
        0,
        &sandbox.last_hash(),
        &Hash::zero(),
        &sandbox.last_state_hash(),
    );

    sandbox.recv(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.assert_lock(Round::zero(), None);
    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.broadcast(&Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.assert_lock(ROUND_ONE, Some(propose.hash()));
}

#[test]
fn test_commit() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &sandbox.last_hash(),
        &[],
        sandbox.s(VALIDATOR_2),
    );

    let block = Block::new(
        SCHEMA_MAJOR_VERSION,
        VALIDATOR_2,
        HEIGHT_ONE,
        0,
        &sandbox.last_hash(),
        &Hash::zero(),
        &sandbox.last_state_hash(),
    );

    sandbox.recv(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.broadcast(&Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.recv(&Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &propose.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.recv(&Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &propose.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    ));
    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
}

#[test]
#[should_panic(expected = "Expected to broadcast the message Consensus(Prevote")]
fn received_unexpected_propose() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(
        VALIDATOR_1,
        Height::zero(),
        ROUND_ONE,
        &sandbox.last_hash(),
        &[],
        sandbox.s(VALIDATOR_1),
    );

    sandbox.recv(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        Height::zero(),
        ROUND_ONE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_0),
    ));
}
