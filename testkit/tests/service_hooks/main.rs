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

#[macro_use]
extern crate exonum;
extern crate exonum_testkit;
extern crate serde;
extern crate serde_json;

use exonum::crypto::{Signature, CryptoHash};
use exonum::blockchain::Schema;
use exonum::helpers::Height;
use exonum::messages::Message;
use exonum_testkit::TestKitBuilder;

mod hooks;
// HACK: Silent "dead_code" warning.
pub use hooks::{HandleCommitService, TxAfterCommit};

#[test]
fn test_handle_commit() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(HandleCommitService)
        .create();
    // Check that `handle_commit` invoked on the correct height.
    for i in 1..5 {
        testkit.create_block();
        let tx = TxAfterCommit::new_with_signature(Height(i), &Signature::zero());
        testkit.add_tx(tx.raw().clone());
        let snapshot = testkit.blockchain_mut().snapshot();
        let schema = Schema::new(&snapshot);
        assert!(schema.transactions().contains(&tx.hash()));
        assert!(schema.transactions_pool().contains(&tx.hash()));
    }
}
