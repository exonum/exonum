// Copyright 2018 The Exonum Team
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

// spell-checker:ignore unsynchronized

mod basic;
mod block_request;
mod config;
mod invalid_message;
mod recovery;
mod round_details;
mod timeouts;
mod transactions;
mod unsynchronized_message;

// - lock to propose when get +2/3 prevote
//     - only if propose is known     - covered in request_propose_when_get_prevote()
//     - only if all txs is known     - covered in request_txs_when_get_propose_or_prevote()
//     - only if round > locked round - covered in lock_to_propose_when_get_2_3_prevote_positive()

// LOCK

// - Send precommit when get lock   //covered in lock_to_propose_when_get_2_3_prevote_positive()
// - if haven’t incompatible prevotes
//     - if has +2/3 precommits
//         TODO: This idea is unreachable because there are no any places in the code
//         where RequestPrecommit is added. (ECR-1627)
//         - remove precommit request
//         - COMMIT //covered in test_reach_one_height
//         - We are fucked up   //covered in handle_full_propose_we_are_fucked_up()
//         - not send prevotes after commit     //covered in lock_not_send_prevotes_after_commit()
// - Send prevote       //covered in lock_to_propose_and_send_prevote()
//     - round > locked + 1     //covered in lock_to_propose_when_get_2_3_prevote_positive
//     - round < current_round  //covered in lock_to_past_round_broadcast_prevote()
//     - not send prevote yet
//     - if get +2/3 prevotes
//         - remove prevote request //covered in lock_remove_request_prevotes
//         - lock to propose        //covered in lock_to_propose_when_get_2_3_prevote_positive

// HANDLE PRECOMMIT

// - Request propose if unknown //covered in commit_using_unknown_propose_with_precommits()
// - Request txs if has unknown //covered in request_txs_when_get_propose_or_prevote()
// - Request prevotes           //covered in commit_using_unknown_propose_with_precommits()
//     - if msg.round > locked round    //covered in handle_precommit_remove_request_prevotes()
// - If has +2/3 precommit      //covered in handle_precommit_positive_scenario_commit()
//     TODO: This idea is unreachable because there are no any places in the code where
//     RequestPrecommit is added. (ECR-1627)
//     - remove precommit request
//     - COMMIT
//         - if propose is known    //covered in do_not_commit_if_propose_is_unknown()
//         - has all txs           //covered in do_not_commit_if_tx_is_unknown()
//         - Our block_hash different from precommits one
//           covered in handle_precommit_different_block_hash()
//     - add_unknown_propose_with_precommits
//       covered in commit_using_unknown_propose_with_precommits()

// COMMIT:

// - update height and lock //is checked by verifying height and lock in
//   handle_precommit_positive_scenario_commit()
// - handle queued messages
//   ?covered in handle_precommit_positive_scenario_commit_with_queued_precommit
// - update blockchain
//   covered in the end of handle_precommit_positive_scenario_commit_with_queued_precommit
// - send propose if is leader //covered in commit_as_leader_send_propose_round_timeout
// - add round timeout          //covered in commit_as_leader_send_propose_round_timeout
// - request commits            //commit message is absent at the moment

// HANDLE TX

// - ignore existed transaction (in both blockchain and pool) because with blockchain is covered
//   in handle_tx_ignore_existing_tx_in_blockchain(); case with pool is not covered because can't
//   create such scenario in terms of messages
// - verify signature   //covered in handle_tx_verify_signature
// - if get full propose:
//     - remove tx request      //covered in not_request_txs_when_get_tx_and_propose
//     - all in handle full propose    //covered in handle_tx_handle_full_propose()

// HANDLE ROUND TIMEOUT:

// - Ignore if height and round are not the same
//   covered in handle_round_timeout_ignore_if_height_and_round_are_not_the_same()
// - increment round
//   covered in handle_round_timeout_increment_round_add_new_round_timeout()
// - add new round timeout
//   covered in handle_round_timeout_increment_round_add_new_round_timeout()
// - send prevote if locked to propose
// - send propose if we are leader
//   covered in test_send_propose_and_prevote_when_we_are_leader()
// - handle queued messages
//   covered in test_handle_round_timeout_queue_prevote_message_from_next_round()

// HANDLE REQUEST TIMEOUT:

// TODO: Investigate how check this and come back when '// FIXME: check height?' is fixed. (ECR-1627)
// - check height?
// - Propose/Tx/Prevotes/Precommits/Commit  //looks like that all these are covered
//      send(RequestPropose):       test_queue_prevote_message_from_next_height
//      recv(RequestTransactions):  response_to_request_txs
// - if we have another known node:
//     - send new request message//for RequestTransaction is covered in handle_tx_handle_full_propose()
//     - add timeout             //for RequestTransaction is covered in handle_tx_handle_full_propose()

// TODO: Add scenario for single node network. (ECR-1627)
