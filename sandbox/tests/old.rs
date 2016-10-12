extern crate exonum;
extern crate sandbox;
extern crate time;

use time::Duration;

use exonum::messages::{Message, Propose, Prevote, Precommit};
use exonum::blockchain::Block;
use exonum::crypto::hash;

use sandbox::timestamping_sandbox;


#[test]
fn test_send_propose_and_prevote() {
    let sandbox = timestamping_sandbox();

    // get some tx
    let tx = sandbox.gen_tx();
    sandbox.recv(tx.clone());

    // round happens
    sandbox.set_time(1, 0);
    sandbox.set_time(2, 0);
    sandbox.set_time(3, 0);

    sandbox.assert_state(0, 4);

    // ok, we are leader
    let propose = Propose::new(0,
                               0,
                               4,
                               sandbox.time() ,
                               &sandbox.last_hash(),
                               &[tx.hash()],
                               sandbox.s(0));

    sandbox.broadcast(propose.clone());
    sandbox.broadcast(Prevote::new(0, 0, 4, &propose.hash(), 0, sandbox.s(0)));
}

#[test]
fn test_send_prevote() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(1,
                               0,
                               1,
                               sandbox.time() + Duration::milliseconds(sandbox.propose_timeout()),
                               &sandbox.last_hash(),
                               &[],
                               sandbox.s(1));

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 0, 1, &propose.hash(), 0, sandbox.s(0)));
}

#[test]
fn test_get_lock_and_send_precommit() {
    let sandbox = timestamping_sandbox();

    let propose_time = sandbox.time() + Duration::milliseconds(sandbox.propose_timeout());
    let propose = Propose::new(1,
                               0,
                               1,
                               propose_time,
                               &sandbox.last_hash(),
                               &[],
                               sandbox.s(1));

    let block = Block::new(0, propose_time, &hash(&[]), &hash(&[]), &hash(&[]), 1);

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 0, 1, &propose.hash(), 0, sandbox.s(0)));
    sandbox.recv(Prevote::new(1, 0, 1, &propose.hash(), 0, sandbox.s(1)));
    sandbox.assert_lock(0, None);
    sandbox.recv(Prevote::new(2, 0, 1, &propose.hash(), 0, sandbox.s(2)));
    sandbox.broadcast(Precommit::new(0, 0, 1, &propose.hash(), &block.hash(), sandbox.s(0)));
    sandbox.assert_lock(1, Some(propose.hash()));
}

#[test]
fn test_commit() {
    let sandbox = timestamping_sandbox();

    let propose_time = sandbox.time() + Duration::milliseconds(sandbox.propose_timeout());
    let propose = Propose::new(1,
                               0,
                               1,
                               propose_time,
                               &sandbox.last_hash(),
                               &[],
                               sandbox.s(1));

    let block = Block::new(0, propose_time, &hash(&[]), &hash(&[]), &hash(&[]), 1);

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 0, 1, &propose.hash(), 0, sandbox.s(0)));
    sandbox.recv(Prevote::new(1, 0, 1, &propose.hash(), 0, sandbox.s(1)));
    sandbox.recv(Prevote::new(2, 0, 1, &propose.hash(), 0, sandbox.s(2)));
    sandbox.broadcast(Precommit::new(0, 0, 1, &propose.hash(), &block.hash(), sandbox.s(0)));
    sandbox.recv(Precommit::new(2, 0, 1, &propose.hash(), &propose.hash(), sandbox.s(2)));
    sandbox.recv(Precommit::new(3, 0, 1, &propose.hash(), &propose.hash(), sandbox.s(3)));
    sandbox.assert_state(0, 1);
}

#[test]
#[should_panic(expected = "Expected to broadcast the message Prevote")]
fn received_unexpected_propose() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(1,
                               0,
                               1,
                               sandbox.time(),
                               &sandbox.last_hash(),
                               &[],
                               sandbox.s(1));

    sandbox.recv(propose.clone());
    sandbox.broadcast(Prevote::new(0, 0, 1, &propose.hash(), 0, sandbox.s(0)));
}