extern crate exonum;
extern crate sandbox;
extern crate time;

use time::Duration;

use exonum::messages::{Message, Propose, Prevote, Precommit};

use sandbox::timestamping_sandbox;

// HANDLE CONSENSUS BASIC

#[test]
fn test_queue_message_from_future_round() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(2,
                               0,
                               2,
                               sandbox.time() + Duration::milliseconds(sandbox.propose_timeout()),
                               &sandbox.last_hash(),
                               &[],
                               sandbox.s(2));

    sandbox.recv(propose.clone());
    sandbox.set_time(0, 999_999_999);
    sandbox.assert_state(0, 1);
    sandbox.set_time(1, 0);
    sandbox.assert_state(0, 2);
    sandbox.broadcast(Prevote::new(0, 0, 2, &propose.hash(), 0, sandbox.s(0)));
}

#[test]
fn test_queue_message_from_next_height() {
    // TODO
}

#[test]
fn test_ignore_message_from_far_height() {
    // TODO
}

#[test]
fn test_ignore_message_from_prev_height() {
    // TODO
}

#[test]
fn test_ignore_message_with_incorrect_validator_id() {
    // TODO
}

#[test]
fn test_ignore_message_with_incorrect_signature() {
    // TODO
}

// HANDLE PROPOSE

#[test]
fn ignore_propose_with_incorrect_prev_hash() {
    // TODO
}

#[test]
fn ignore_propose_from_non_leader() {
    // TODO
}

#[test]
fn ignore_propose_with_incorrect_time() {
    // TODO
}

#[test]
fn ignore_propose_with_commited_transaction() {
    // TODO
}


// - remove propose request when getting propose and request txs from known nodes
//     - not only leader, also prevotes
//     - not only leader, alto precommiters
// - request tx from propose with unkwnown tx
// - ignore propose that sends before than timeout exceeded 

// HAS FULL PROPOSE

// - send prevote when lock=0 for known propose
// - not send prevote if lock > 0

// - get lock when getting propose if we have +2/3 prevotes (consensus:88)
//     - only after lock
//     - only after propose_round
//     - only before current round

// - commit when getting propose if we have +2/3 precommits (consensus:96)
// - panic when getting propose if we have +2/3 precommits (consensus:96)

// HANDLE PREVOTE

// - request propose when get prevote
// - request txs when get prevote
// - request prevotes when get prevote message
// - remove prevote request when getting prevote
// - lock to propose when get +2/3 prevote
//     - only if propose is known
//     - only if all txs is known
//     - only if round > locked round

// LOCK

// - Send precommit when get lock
// - if haven’t incompatible prevotes
//     - if has +2/3 precommits
//         - remove precommit request
//         - COMMIT
//         - We are fucked up
//         - not send prevotes after commit
// - Send prevote
//     - round > locked + 1
//     - round < current_round
//     - not send prevote yet
//     - if get +2/3 prevotes
//         - remove prevote request
//         - lock to propose

// HANDLE PRECOMMIT

// - Request propose if unknown
// - Request txs if has unknown
// - Request prevotes
//     - if msg.round > locked round
// - If has +2/3 precommit
//     - remove precommit request
//     - COMMIT
//         - if propose is known
//         - has all txs
//         - We are fucked up
//     - add_unknown_propose_with_precommits?

// COMMIT:

// - update height and lock
// - handle queued messages
// - update blockchain
// - send proopose if is leader
// - add round timeout
// - request commits

// HANDLE TX

// - ignore existed transaction (in both blockchain and pool)
// - verify signature
// - if get full propose:
//     - remove tx request
//     - all in has full propose

// HANDLE ROUND TIMEOUT:

// - Ignore if height and round are not the same
// - increment round
// - add new round timeout
// - send prevote if locked to propose
// - send propose if we are leader
// - handle queued messages

// HANDLE REQUEST TIMEOUT:

// - check height?
// - Propose/Tx/Prevotes/Precommits/Commit
// - if we have another known node:
//     - send new request message
//     - add timeout
