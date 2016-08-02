use super::sandbox::Sandbox;
use super::messages::{Message, Propose, Prevote, Precommit, Commit};

#[test]
fn test_basic_send_prevote() {
    let sandbox = Sandbox::new();

    let propose = Propose::new(1, 1, 1, sandbox.time(), &sandbox.last_hash(), &[], sandbox.v1());

    sandbox.send(propose.clone());
    sandbox.recv(Prevote::new(0, 1, 1, &propose.hash(), 0, sandbox.v0()));
}
