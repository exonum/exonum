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
    crypto::Hash,
    helpers::{Height, ValidatorId},
};

use std::time::Duration;

use crate::sandbox::{sandbox_tests_helper::gen_timestamping_tx, timestamping_sandbox, Sandbox};

// Receive status with pool_size = 1
// Send pool transactions request
// Receive response with one tx
#[test]
fn pool_transactions_request() {
    let sandbox = timestamping_sandbox();
    let tx1 = gen_timestamping_tx();

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(1)),
        Height(1),
        Hash::zero(),
        1,
        sandbox.secret_key(ValidatorId(1)),
    ));

    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));

    let request = Sandbox::create_pool_transactions_request(
        sandbox.public_key(ValidatorId(0)),
        sandbox.public_key(ValidatorId(1)),
        sandbox.secret_key(ValidatorId(0)),
    );

    sandbox.send(sandbox.public_key(ValidatorId(1)), &request);
    sandbox.recv(&tx1);
    sandbox.assert_tx_cache_len(1);

    sandbox.recv(&Sandbox::create_transactions_response(
        sandbox.public_key(ValidatorId(1)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx1],
        sandbox.secret_key(ValidatorId(1)),
    ));
}
