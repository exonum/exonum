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

// HACK: Silent "dead_code" warning.
pub use hooks::{AfterCommitService, TxAfterCommit};

use exonum::{
    crypto::{CryptoHash, Signature}, helpers::Height, messages::Message,
};
use exonum_testkit::TestKitBuilder;

mod hooks;

#[test]
fn test_after_commit() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(AfterCommitService)
        .create();

    // Check that `after_commit` invoked on the correct height.
    for i in 1..5 {
        let block = testkit.create_block();
        if i > 1 {
            assert_eq!(
                block[0].content().raw(),
                TxAfterCommit::new_with_signature(Height(i - 1), &Signature::zero()).raw()
            );
        }

        let tx = TxAfterCommit::new_with_signature(Height(i), &Signature::zero());
        assert!(testkit.is_tx_in_pool(&tx.hash()));
    }

    let expected_block_sizes = testkit
        .explorer()
        .blocks(Height(1)..)
        .all(|block| block.len() == if block.height() == Height(1) { 0 } else { 1 });
    assert!(expected_block_sizes);
}
