use exonum::messages::{Message, Propose, Prevote, Precommit};

use super::sandbox::timestamping_sandbox;

// Send propose

#[test]
fn test_send_propose_and_prevote() {
    let sandbox = timestamping_sandbox();

    // round happens
    sandbox.set_time(1, 0);
    sandbox.set_time(2, 0);
    sandbox.set_time(3, 0);

    sandbox.assert_state(0, 4);

    // ok, we are leader
    let propose = Propose::new(0, 0, 4,
                               sandbox.time(),
                               &sandbox.last_hash(),
                               &[],
                               sandbox.s(0));

    sandbox.broadcast(propose.clone());
    sandbox.broadcast(Prevote::new(0, 0, 4, &propose.hash(), 0, sandbox.s(0)));
}

#[test]
fn test_send_prevote() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(1, 0, 1, sandbox.time(), &sandbox.last_hash(), &[], sandbox.s(1));

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 0, 1, &propose.hash(), 0, sandbox.s(0)));
}

#[test]
fn test_get_lock_and_send_precommit() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(1, 0, 1, sandbox.time(), &sandbox.last_hash(), &[], sandbox.s(1));

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 0, 1, &propose.hash(), 0, sandbox.s(0)));
    sandbox.recv(Prevote::new(1, 0, 1, &propose.hash(), 0, sandbox.s(1)));
    sandbox.assert_lock(0, None);
    sandbox.recv(Prevote::new(2, 0, 1, &propose.hash(), 0, sandbox.s(2)));
    sandbox.broadcast(Precommit::new(0, 0, 1, &propose.hash(), &propose.hash(), sandbox.s(0)));
    sandbox.assert_lock(1, Some(propose.hash()));
}

#[test]
fn test_commit() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(1, 0, 1, sandbox.time(), &sandbox.last_hash(), &[], sandbox.s(1));

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 0, 1, &propose.hash(), 0, sandbox.s(0)));
    sandbox.recv(Prevote::new(1, 0, 1, &propose.hash(), 0, sandbox.s(1)));
    sandbox.recv(Prevote::new(2, 0, 1, &propose.hash(), 0, sandbox.s(2)));
    sandbox.broadcast(Precommit::new(0, 0, 1, &propose.hash(), &propose.hash(), sandbox.s(0)));
    sandbox.recv(Precommit::new(2, 0, 1, &propose.hash(), &propose.hash(), sandbox.s(2)));
    sandbox.recv(Precommit::new(3, 0, 1, &propose.hash(), &propose.hash(), sandbox.s(3)));
    sandbox.assert_state(1, 1);
}

// TODO: add test by consensus.rs
// TODO: add test by state.rs
// TODO: тесты на идемпотентность
// TODO: fuzz test: получение сообщений в разном порядке приводит к одному и тому же состоянию
// TODO: test byzantine behavior
// TODO: test incorrect message in some way (incorrect sign, validator id, etc)

// fn handle
// fn handle_propose
// fn has_full_propose
// fn handle_prevote
// fn has_majority_prevotes
// fn has_majority_precommits
// fn lock
// fn handle_precommit
// fn commit
// fn handle_tx
// fn handle_commit
// fn handle_round_timeout
// fn handle_request_timeout
// fn is_leader
// fn execute
// fn request_propose_or_txs
// fn request
// fn remove_request
// fn send_propose
// fn send_prevote
// fn send_precommit
// fn send_commit
