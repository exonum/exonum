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

use exonum::{
    helpers::{Height, Round, ValidatorId},
    merkledb::ObjectHash,
};

use crate::{
    messages::{Message, Service},
    sandbox::{sandbox_tests_helper::*, timestamping_sandbox},
};

#[test]
fn approving_skip_propose_normal_workflow() {
    let sandbox = timestamping_sandbox();
    let propose = sandbox.create_skip_propose(
        ValidatorId(2),
        Height(1),
        Round(1),
        sandbox.last_hash(),
        sandbox.secret_key(ValidatorId(2)),
    );
    sandbox.recv(&propose);
    let propose_hash = propose.object_hash();

    // Since the node has all 0 transactions from the `Propose`, it should vote for it.
    let our_prevote = make_prevote_from_propose(&sandbox, &propose);
    sandbox.broadcast(&our_prevote);

    // Receive prevotes and precommits from other nodes.
    let prevotes = (2..4).map(|i| {
        let validator = ValidatorId(i);
        sandbox.create_prevote(
            validator,
            Height(1),
            Round(1),
            propose_hash,
            NOT_LOCKED,
            sandbox.secret_key(validator),
        )
    });
    for prevote in prevotes {
        sandbox.recv(&prevote);
    }

    let block = sandbox.create_block_skip();
    let block_hash = block.object_hash();
    let our_precommit = sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose_hash,
        block_hash,
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&our_precommit);

    let precommits = (1..3).map(|i| {
        let validator = ValidatorId(i);
        sandbox.create_precommit(
            validator,
            Height(1),
            Round(1),
            propose_hash,
            block_hash,
            sandbox.time().into(),
            sandbox.secret_key(validator),
        )
    });
    for precommit in precommits {
        sandbox.recv(&precommit);
    }

    sandbox.assert_state(Height(2), Round(1));
    assert_eq!(sandbox.node_state().blockchain_height(), Height(1));
    let our_status = sandbox.create_our_status(Height(2), Height(1), 0);
    sandbox.broadcast(&our_status);

    // Check that the epoch is preserved across node restarts.
    let current_time = sandbox.time();
    let sandbox = sandbox.restart_with_time(current_time);
    sandbox.assert_state(Height(2), Round(1));
}

#[derive(Debug, Clone, Copy)]
enum MessageType {
    Propose,
    Prevote,
    Precommit,
}

fn test_approving_skip_propose(sequence: &[MessageType]) {
    let sandbox = timestamping_sandbox();

    let propose = sandbox.create_skip_propose(
        ValidatorId(2),
        Height(1),
        Round(1),
        sandbox.last_hash(),
        sandbox.secret_key(ValidatorId(2)),
    );
    let propose_hash = propose.object_hash();

    let mut prevotes = (1..4).map(|i| {
        let validator = ValidatorId(i);
        sandbox.create_prevote(
            validator,
            Height(1),
            Round(1),
            propose_hash,
            NOT_LOCKED,
            sandbox.secret_key(validator),
        )
    });

    let block = sandbox.create_block_skip();
    let block_hash = block.object_hash();
    let mut precommits = (1..4).map(|i| {
        let validator = ValidatorId(i);
        sandbox.create_precommit(
            validator,
            Height(1),
            Round(1),
            propose_hash,
            block_hash,
            sandbox.time().into(),
            sandbox.secret_key(validator),
        )
    });

    for &message_type in sequence {
        match message_type {
            MessageType::Propose => sandbox.recv(&propose),
            MessageType::Prevote => sandbox.recv(&prevotes.next().unwrap()),
            MessageType::Precommit => sandbox.recv(&precommits.next().unwrap()),
        }

        // We don't care about intermediate messages in this test, but need to poll them
        // in order for `recv` not to panic.
        while let Some((_, msg)) = sandbox.pop_sent_message() {
            if let Message::Service(Service::Status(status)) = msg {
                let our_status = sandbox.create_our_status(Height(2), Height(1), 0);
                assert_eq!(status, our_status);

                // Remove remaining messages from the queue.
                while sandbox.pop_sent_message().is_some() {}
                return;
            }
        }
    }

    panic!("Haven't reached new epoch by the end of the test");
}

#[test]
fn approve_skip_propose_a() {
    use self::MessageType::*;
    test_approving_skip_propose(&[Propose, Prevote, Prevote, Precommit, Precommit]);
}

#[test]
fn approve_skip_propose_b() {
    use self::MessageType::*;
    test_approving_skip_propose(&[Prevote, Prevote, Propose, Precommit, Precommit]);
}

#[test]
fn approve_skip_propose_c() {
    use self::MessageType::*;
    test_approving_skip_propose(&[Prevote, Prevote, Precommit, Propose, Precommit]);
}

#[test]
fn approve_skip_propose_d() {
    use self::MessageType::*;
    test_approving_skip_propose(&[Precommit, Precommit, Prevote, Prevote, Propose]);
}

#[test]
fn approve_skip_propose_e() {
    use self::MessageType::*;
    test_approving_skip_propose(&[Propose, Precommit, Precommit, Prevote, Prevote]);
}

#[test]
fn approve_skip_propose_f() {
    use self::MessageType::*;
    test_approving_skip_propose(&[Prevote, Propose, Precommit, Prevote, Precommit]);
}

#[test]
fn approve_skip_propose_other_precommits_a() {
    use self::MessageType::*;
    test_approving_skip_propose(&[Precommit, Precommit, Precommit, Propose]);
}

#[test]
fn approve_skip_propose_other_precommits_b() {
    use self::MessageType::*;
    test_approving_skip_propose(&[Precommit, Precommit, Propose, Precommit]);
}

#[test]
fn approve_skip_propose_other_precommits_c() {
    use self::MessageType::*;
    test_approving_skip_propose(&[Propose, Precommit, Precommit, Precommit]);
}

#[test]
fn approve_skip_propose_other_precommits_d() {
    use self::MessageType::*;
    test_approving_skip_propose(&[Propose, Precommit, Precommit, Prevote, Precommit]);
}
