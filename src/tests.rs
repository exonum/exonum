// use super::sandbox::Sandbox;
// use super::messages::{Message, Propose, Prevote, Precommit, Commit};

// #[test]
// fn test_basic_send_prevote() {
//     let mut sandbox = Sandbox::new();

//     // let propose = Propose::new(1, 1, 1, sandbox.time(), &sandbox.last_hash(), &[], sandbox.s(1));

//     // sandbox.send(sandbox.a(1), propose.clone());
//     // sandbox.recv(Prevote::new(0, 1, 1, &propose.hash(), 0, sandbox.s(0)));
// }

// TODO: add test by consensus.rs
// TODO: add test by state.rs
// TODO: тесты на идемпотентность
// TODO: fuzz test: получение сообщений в разном порядке приводит к одному и тому же состоянию

// send propose
// send prevote
// send precommit
// send status

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
