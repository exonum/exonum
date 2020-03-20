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

//! Tests in this module are designed to test ability of the node to handle
//! message that arrive at the wrong time.

use exonum::{
    blockchain::ProposerId,
    crypto::Hash,
    helpers::{Height, Round, ValidatorId},
    merkledb::ObjectHash,
};
use log::info;

use std::time::Duration;

use crate::sandbox::{sandbox_tests_helper::*, timestamping_sandbox};

#[test]
fn test_queue_message_from_future_round() {
    let sandbox = timestamping_sandbox();

    let propose = sandbox.create_propose(
        ValidatorId(3),
        Height(1),
        Round(2),
        sandbox.last_hash(),
        vec![],
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout() - 1));
    sandbox.assert_state(Height(1), Round(1));
    sandbox.add_time(Duration::from_millis(1));
    sandbox.assert_state(Height(1), Round(2));
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(2),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// idea of the scenario is to:
/// - receive correct Prevote for some next height (first one) at 0 time (and respectively 1 height)
/// - queue it
/// - reach that first height
/// - handle queued Prevote
/// - and observe `ProposeRequest` for queued `Prevote`
#[test]
#[should_panic(expected = "Sent unexpected message Requests(ProposeRequest")]
fn test_queue_prevote_message_from_next_height() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(3),
        Height(2),
        Round(1),
        Hash::zero(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(3)),
    ));

    add_one_height(&sandbox, &sandbox_state);
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout() - 1));
    sandbox.add_time(Duration::from_millis(0));
}

/// idea of the scenario is to:
/// - receive correct Propose for some next height (first one) at 0 time (and respectively 1 height)
/// - queue it
/// - reach that first height
/// - handle queued Propose
/// - and observe Prevote for queued Propose
/// check line from `NodeHandler.handle_consensus()`
/// case `msg.height() == self.state.height() + 1`
#[test]
#[should_panic(expected = "Sent unexpected message Consensus(Prevote")]
fn test_queue_propose_message_from_next_height() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();
    let tx = gen_timestamping_tx();
    let mut block_at_first_height = sandbox.create_block(&[tx.clone()]);
    block_at_first_height.add_header::<ProposerId>(ValidatorId(0));

    let future_propose = sandbox.create_propose(
        ValidatorId(0),
        Height(2),
        Round(2),
        block_at_first_height.clone().object_hash(),
        vec![], // there are no transactions in future propose
        sandbox.secret_key(ValidatorId(0)),
    );

    sandbox.recv(&future_propose);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx]);

    info!(
        "last_block={:#?}, hash={:?}",
        sandbox.last_block(),
        sandbox.last_block().object_hash()
    );
    info!(
        "proposed_block={:#?}, hash={:?}",
        block_at_first_height,
        block_at_first_height.object_hash()
    );

    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    sandbox.add_time(Duration::from_millis(0));
}

/// idea of scenario is to check line // Ignore messages from previous and future height
/// from `NodeHandler.handle_consensus()`
/// case `msg.height() > self.state.height() + 1`
#[test]
fn test_ignore_message_from_far_height() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_height(Height(2)) //without this line some Prevote will be sent
        .build();

    sandbox.recv(&propose);
}

/// idea of scenario is to check line // Ignore messages from previous and future height
/// from `NodeHandler.handle_consensus()`
/// case `msg.height() < self.state.height()`
#[test]
fn test_ignore_message_from_prev_height() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);

    sandbox.assert_state(Height(2), Round(1));

    let propose = ProposeBuilder::new(&sandbox)
        .with_height(Height(0)) //without this line some Prevote will be sent
        .build();

    sandbox.recv(&propose);
}
