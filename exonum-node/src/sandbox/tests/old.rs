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

use std::time::Duration;

use exonum::{
    blockchain::{AdditionalHeaders, Block, Epoch, ProposerId},
    helpers::{Height, Round, ValidatorId},
    merkledb::{HashTag, ObjectHash},
    messages::Verified,
};

use crate::messages::Propose;
use crate::sandbox::{
    sandbox_tests_helper::{gen_timestamping_tx, NOT_LOCKED},
    timestamping_sandbox, Sandbox,
};

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
        sandbox.last_hash(),
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(0)),
    );

    sandbox.broadcast(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(3),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

fn create_propose(sandbox: &Sandbox) -> Verified<Propose> {
    sandbox.create_propose(
        ValidatorId(2),
        Height(1),
        Round(1),
        sandbox.last_hash(),
        vec![],
        sandbox.secret_key(ValidatorId(2)),
    )
}

fn create_block(sandbox: &Sandbox) -> Block {
    let mut additional_headers = AdditionalHeaders::new();
    additional_headers.insert::<ProposerId>(ValidatorId(2));
    additional_headers.insert::<Epoch>(sandbox.current_epoch());

    Block {
        height: Height(1),
        tx_count: 0,
        prev_hash: sandbox.last_hash(),
        tx_hash: HashTag::empty_list_hash(),
        state_hash: sandbox.last_state_hash(),
        error_hash: HashTag::empty_map_hash(),
        additional_headers,
    }
}

#[test]
fn test_send_prevote() {
    let sandbox = timestamping_sandbox();
    let propose = create_propose(&sandbox);

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

#[test]
fn test_get_lock_and_send_precommit() {
    let sandbox = timestamping_sandbox();
    let propose = create_propose(&sandbox);
    let block = create_block(&sandbox);

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None);
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.broadcast(&sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    ));
    sandbox.assert_lock(Round(1), Some(propose.object_hash()));
}

#[test]
fn test_commit() {
    let sandbox = timestamping_sandbox();
    let propose = create_propose(&sandbox);
    let block = create_block(&sandbox);

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.broadcast(&sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    ));
    sandbox.recv(&sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        propose.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.recv(&sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        propose.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    ));
    sandbox.assert_state(Height(1), Round(1));
}

#[test]
#[should_panic(expected = "Expected to broadcast the message")]
fn received_unexpected_propose() {
    let sandbox = timestamping_sandbox();

    let propose = sandbox.create_propose(
        ValidatorId(1),
        Height::zero(),
        Round(1),
        sandbox.last_hash(),
        vec![],
        sandbox.secret_key(ValidatorId(1)),
    );

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height::zero(),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));
}
