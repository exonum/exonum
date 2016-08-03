use super::sandbox::Sandbox;
use super::messages::{Message, Propose, Prevote, Precommit, Commit};

// #[test]
// fn test_basic_send_prevote() {
//     let mut sandbox = Sandbox::new();

//     // let propose = Propose::new(1, 1, 1, sandbox.time(), &sandbox.last_hash(), &[], sandbox.s(1));

//     // sandbox.send(sandbox.a(1), propose.clone());
//     // sandbox.recv(Prevote::new(0, 1, 1, &propose.hash(), 0, sandbox.s(0)));
// }

// TODO: add self test (every panics)
// TODO: add test by consensus.rs
// TODO: add test by state.rs
