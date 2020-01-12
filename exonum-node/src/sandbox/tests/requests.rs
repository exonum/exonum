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

// =======================

// HANDLE REQUEST

// - ignore if to incorrect
// - ignore if incorrect time
// - ignore if time < 0
// - ignore if time > REQUEST_ALIVE
// - ignore if incorrect signature

// REQUEST PROPOSE:
// - ignore if wrong height
// - ignore if hasn’t propose
// - send propose

// REQUEST TXS:
// - ignore if hasn’t
// - send from pool
// - send from blockchain

// REQUEST PREVOTES:
// - ignore if height != our height
// - send prevotes we have (> +2/3, <+2/3, 0)

// REQUEST PRECOMMITS:
// - ignore if height > our height
// - send precommits we have (> +2/3, <+2/3, 0) for out height
// - send precommits from blockchain for prev height if we have (or not send if haven’t)

// REQUEST COMMIT:
// - ignore if height = our height
// - ignore if height > our height
// - send +2/3 precommits if we have (not send if haven’t)

// BYZANTINE:
// - get precommits with different block_hash
// - send different proposes
// - not send proposes
// - update lock

use exonum::{
    crypto::Hash,
    helpers::{Height, ValidatorId},
};

use std::time::Duration;

use crate::sandbox::{sandbox_tests_helper::gen_timestamping_tx, timestamping_sandbox};

// Receive status with pool_size = 1
// Send pool transactions request
// Receive response with one tx
#[test]
fn pool_transactions_request() {
    let sandbox = timestamping_sandbox();
    let tx1 = gen_timestamping_tx();

    sandbox.recv(&sandbox.create_status(
        sandbox.public_key(ValidatorId(1)),
        Height(1),
        Hash::zero(),
        1,
        sandbox.secret_key(ValidatorId(1)),
    ));

    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));

    let request = sandbox.create_pool_transactions_request(
        sandbox.public_key(ValidatorId(0)),
        sandbox.public_key(ValidatorId(1)),
        sandbox.secret_key(ValidatorId(0)),
    );

    sandbox.send(sandbox.public_key(ValidatorId(1)), &request);
    sandbox.recv(&tx1);
    sandbox.assert_tx_cache_len(1);

    sandbox.recv(&sandbox.create_transactions_response(
        sandbox.public_key(ValidatorId(1)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx1],
        sandbox.secret_key(ValidatorId(1)),
    ));
}
