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
//! incorrect messages.

use exonum::{
    crypto::Hash,
    helpers::{Height, Round, ValidatorId},
    merkledb::ObjectHash,
    messages::Verified,
};

use crate::{
    messages::Propose,
    sandbox::{sandbox_tests_helper::*, timestamping_sandbox},
};

/// HANDLE message
/// - verify signature
/// - sandbox should panic on message verification

#[test]
#[should_panic]
fn test_ignore_message_with_incorrect_signature() {
    let sandbox = timestamping_sandbox();

    let propose = sandbox.create_propose(
        ValidatorId(0),
        Height(0),
        Round(1),
        sandbox.last_hash(),
        vec![],
        sandbox.secret_key(ValidatorId(1)),
    );

    sandbox.recv(&propose);
}

#[test]
fn test_ignore_message_with_incorrect_validator_id() {
    let sandbox = timestamping_sandbox();

    let incorrect_validator_id = ValidatorId(64_999);

    let propose = Verified::from_value(
        Propose::new(
            incorrect_validator_id,
            Height(0),
            Round(1),
            sandbox.last_hash(),
            vec![],
        ),
        sandbox.public_key(ValidatorId(1)),
        sandbox.secret_key(ValidatorId(1)),
    );

    sandbox.recv(&propose);
}

// HANDLE PROPOSE

#[test]
fn ignore_propose_with_incorrect_prev_hash() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_prev_hash(&Hash::zero())
        .build();

    sandbox.recv(&propose);
}

#[test]
fn ignore_propose_from_non_leader() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_validator(ValidatorId(3)) //without this line Prevote would have been broadcast
        .build();

    sandbox.recv(&propose);
}

/// Propose with incorrect time should be handled as usual.
#[test]
fn handle_propose_with_incorrect_time() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox).build();

    sandbox.recv(&propose);

    sandbox.assert_lock(NOT_LOCKED, None);
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
fn ignore_propose_with_committed_transaction() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);

    let propose = ProposeBuilder::new(&sandbox)
        // without this line Prevote would have been broadcast
        .with_tx_hashes(sandbox_state.committed_transaction_hashes.borrow().as_ref())
        .build();

    sandbox.recv(&propose);
    //    broadcast here is absent
}

#[test]
fn handle_propose_that_sends_before_than_propose_timeout_exceeded() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox).build();

    sandbox.recv(&propose);

    sandbox.assert_lock(NOT_LOCKED, None);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));
}
