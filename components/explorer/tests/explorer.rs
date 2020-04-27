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

//! Tests for the blockchain explorer functionality.

use exonum::{
    blockchain::TxLocation,
    crypto::{Hash, KeyPair},
    helpers::Height,
    merkledb::ObjectHash,
    messages::{AnyTx, Verified},
    runtime::{ErrorKind, SnapshotExt},
};
use serde_json::json;

use std::iter;

use exonum_explorer::*;

mod blockchain;
use self::blockchain::{
    create_block, create_blockchain, CreateWallet, ExplorerTransactions, Transfer, SERVICE_ID,
};

#[test]
#[allow(clippy::cognitive_complexity)]
fn test_explorer_basics() {
    let mut blockchain = create_blockchain();
    let alice = KeyPair::random();
    let bob = KeyPair::random();

    let tx_alice = alice.create_wallet(SERVICE_ID, CreateWallet::new("Alice"));
    let tx_bob = bob.create_wallet(SERVICE_ID, CreateWallet::new("Bob"));
    let tx_transfer = alice.transfer(SERVICE_ID, Transfer::new(bob.public_key(), 2));

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    assert_eq!(explorer.height(), Height(0));
    let block = explorer.block(Height(0)).unwrap();
    assert_eq!(block.len(), 0);
    assert!(block.transaction(0).is_none());
    assert!(explorer.transaction(&tx_alice.object_hash()).is_none());

    // Block #1: Alice's transaction.
    create_block(&mut blockchain, vec![tx_alice.clone()]);
    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    assert_eq!(explorer.height(), Height(1));
    assert_eq!(snapshot.for_core().transactions_len(), 1);
    assert!(explorer.block(Height(2)).is_none());

    let block = explorer.block(Height(1)).unwrap();
    assert_eq!(block.len(), 1);
    let tx_info = block.transaction(0).unwrap();
    assert_eq!(*tx_info.location(), TxLocation::new(Height(1), 0));
    tx_info.status().unwrap();
    assert_eq!(tx_info.message(), &tx_alice);
    assert_eq!(
        tx_info.message().object_hash(),
        block.transaction_hashes()[0]
    );

    let tx_info = explorer.transaction(&tx_alice.object_hash()).unwrap();
    assert!(!tx_info.is_in_pool());
    assert!(tx_info.is_committed());
    assert_eq!(tx_info.message(), &tx_alice);

    let tx_info = match tx_info {
        TransactionInfo::Committed(info) => info,
        tx => panic!("{:?}", tx),
    };
    assert_eq!(*tx_info.location(), TxLocation::new(Height(1), 0));
    assert_eq!(
        serde_json::to_value(&tx_info).unwrap(),
        json!({
            "message": tx_alice,
            "location": {
                "block_height": 1,
                "position_in_block": 0,
            },
            "location_proof": tx_info.location_proof(), // too complicated to check
            "status": { "type": "success" },
            "time": tx_info.time(),
        })
    );

    // Block #2: other transactions.
    create_block(&mut blockchain, vec![tx_bob.clone(), tx_transfer.clone()]);

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    assert_eq!(explorer.height(), Height(2));
    assert_eq!(snapshot.for_core().transactions_len(), 3);
    let block = explorer.block(Height(2)).unwrap();
    assert_eq!(block.len(), 2);

    let tx_info = block.transaction(0).unwrap();
    let err = tx_info.status().unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Service { code: 0 });
    assert_eq!(err.description(), "Not allowed!");
    assert_eq!(
        serde_json::to_value(&tx_info).unwrap(),
        json!({
            "message": tx_bob,
            "location": {
                "block_height": 2,
                "position_in_block": 0,
            },
            "location_proof": tx_info.location_proof(), // too complicated to check
            "status": {
                "type": "service_error",
                "code": 0,
                "description": "Not allowed!",
                "runtime_id": 0,
                "call_site": {
                    "call_type": "method",
                    "instance_id": SERVICE_ID,
                    "method_id": 0,
                },
            },
            "time": tx_info.time(),
        })
    );

    let tx_info = block.transaction(1).unwrap();
    let err = tx_info.status().unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Unexpected);
    assert_eq!(err.description(), "oops");
    assert_eq!(
        serde_json::to_value(&tx_info).unwrap(),
        json!({
            "message": tx_transfer,
            "location": {
                "block_height": 2,
                "position_in_block": 1,
            },
            "location_proof": tx_info.location_proof(), // too complicated to check
            "status": {
                "type": "unexpected_error",
                "description": "oops",
                "runtime_id": 0,
                "call_site": {
                    "call_type": "method",
                    "instance_id": SERVICE_ID,
                    "method_id": 1,
                },
            },
            "time": tx_info.time(),
        })
    );
}

#[test]
fn test_explorer_errors_in_block() {
    let mut blockchain = create_blockchain();
    let bob = KeyPair::random();
    let tx_bob = bob.create_wallet(SERVICE_ID, CreateWallet::new("Bob"));

    create_block(&mut blockchain, vec![tx_bob]);
    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(&snapshot);
    let block = explorer.block_with_txs(Height(1)).unwrap();
    assert_eq!(block.len(), 1);
    assert_eq!(block.errors.len(), 1);
    assert_eq!(
        serde_json::to_value(&block).unwrap(),
        json!({
            "block": {
                "height": 1,
                "tx_count": 1,
                "prev_hash": block.header.prev_hash,
                "tx_hash": block.header.tx_hash,
                "state_hash": block.header.state_hash,
                "error_hash": block.header.error_hash,
                "additional_headers": block.header.additional_headers,
            },
            "precommits": block.precommits,
            "transactions": [
                block[0], // We aren't that interested in the transaction contents in this test
            ],
            "errors": [{
                "error": {
                    "call_site": {
                        "call_type": "method",
                        "instance_id": SERVICE_ID,
                        "method_id": 0
                    },
                    "code": 0,
                    "description": "Not allowed!",
                    "runtime_id": 0,
                    "type": "service_error"
                },
                "location": {
                    "index": 0,
                    "type": "transaction"
                }
            }],
        })
    );
}

#[test]
fn test_explorer_pool_transaction() {
    let mut blockchain = create_blockchain();
    let alice = KeyPair::random();
    let tx_alice = alice.create_wallet(SERVICE_ID, CreateWallet::new("Alice"));
    let tx_hash = tx_alice.object_hash();

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    assert!(explorer.transaction(&tx_hash).is_none());

    blockchain.add_transactions_into_pool(iter::once(tx_alice.clone()));

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    let tx_info = explorer.transaction(&tx_hash).unwrap();
    assert!(tx_info.is_in_pool());
    assert!(!tx_info.is_committed());
    assert_eq!(tx_info.message(), &tx_alice);
}

fn tx_generator() -> impl Iterator<Item = Verified<AnyTx>> {
    (0..).map(|i| {
        KeyPair::random().create_wallet(SERVICE_ID, CreateWallet::new(format!("Alice #{}", i)))
    })
}

// TODO Implement method id getter in CreateWallet. [ECR-3254]
fn is_create_wallet(tx: &CommittedTransaction) -> bool {
    let raw_tx = tx.message().payload();
    if raw_tx.call_info.method_id == 0 {
        raw_tx
            .parse::<CreateWallet>()
            .expect("Unable to parse transaction");
        true
    } else {
        false
    }
}

#[test]
fn test_explorer_block_iter() {
    let mut blockchain = create_blockchain();
    let mut tx_gen = tx_generator();
    let txs = tx_gen.by_ref();

    create_block(&mut blockchain, vec![]); // Height(1)
    create_block(&mut blockchain, txs.take(2).collect()); // Height(2)
    create_block(&mut blockchain, vec![]); // Height(3)
    create_block(&mut blockchain, txs.take(2).collect()); // Height(4)
    create_block(&mut blockchain, txs.take(3).collect()); // Height(5)
    create_block(&mut blockchain, vec![]); // Height(6)
    create_block(&mut blockchain, vec![]); // Height(7)
    create_block(&mut blockchain, vec![]); // Height(8)
    create_block(&mut blockchain, txs.take(5).collect()); // Height(9)
    assert_eq!(blockchain.as_ref().last_block().height, Height(9));

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());

    let mut count = 0;
    for (i, block) in explorer.blocks(..).enumerate() {
        assert_eq!(block.height(), Height(i as u64));
        count += 1;
    }
    assert_eq!(count, 10);

    let tx_count: usize = explorer.blocks(..Height(1_000)).map(|bl| bl.len()).sum();
    assert_eq!(tx_count, 12);

    let count = explorer.blocks(..).filter(BlockInfo::is_empty).count();
    assert_eq!(count, 6);

    let transaction_hashes: Vec<Hash> = explorer
        .blocks(..)
        .flat_map(|info| info.transaction_hashes().to_vec())
        .collect();
    assert_eq!(transaction_hashes.len(), 12);

    let block = explorer.block(Height(4)).unwrap();
    assert_eq!(transaction_hashes[3], block.transaction_hashes()[1]);

    let transactions: Vec<CommittedTransaction> = explorer
        .blocks(..)
        .flat_map(|info| info.with_transactions().transactions)
        .collect();
    assert_eq!(transactions.len(), 12);
    assert!(transactions
        .iter()
        .all(|tx| tx.location().block_height() < Height(10)));

    let heights: Vec<_> = explorer
        .blocks(..)
        .rev()
        .filter(|bl| !bl.is_empty())
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![Height(9), Height(5), Height(4), Height(2)]);

    let block = explorer
        .blocks(..)
        .filter(|bl| !bl.is_empty())
        .nth(2)
        .unwrap();
    assert_eq!(block.height(), Height(5));

    let mut iter = explorer.blocks(..).filter(|bl| !bl.is_empty()).rev();
    let _ = iter.by_ref().nth(1);
    let heights: Vec<_> = iter.by_ref().map(|bl| bl.height()).collect();
    assert_eq!(heights, vec![Height(4), Height(2)]);
    assert!(iter.next().is_none());

    let heights: Vec<_> = explorer
        .blocks(..Height(9))
        .filter(|bl| !bl.is_empty())
        .rev()
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![Height(5), Height(4), Height(2)]);

    let heights: Vec<_> = explorer
        .blocks(..Height(5))
        .filter(|bl| !bl.is_empty())
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![Height(2), Height(4)]);

    let heights: Vec<_> = explorer
        .blocks(Height(0)..Height(2))
        .filter(|bl| !bl.is_empty())
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![]);

    let heights: Vec<_> = explorer.blocks(..Height(2)).map(|bl| bl.height()).collect();
    assert_eq!(heights, vec![Height(0), Height(1)]);

    let heights: Vec<_> = explorer.blocks(..Height(1)).map(|bl| bl.height()).collect();
    assert_eq!(heights, vec![Height(0)]);

    let heights: Vec<_> = explorer.blocks(..Height(0)).map(|bl| bl.height()).collect();
    assert_eq!(heights, vec![]);

    let heights: Vec<_> = explorer
        .blocks(Height(0)..Height(1))
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![Height(0)]);

    let mut iter = explorer.blocks(..);
    assert_eq!(iter.by_ref().nth(3).unwrap().height(), Height(3));
    assert_eq!(iter.nth(2).unwrap().height(), Height(6));
}

#[test]
fn test_transaction_iterator() {
    let mut blockchain = create_blockchain();
    let txs = tx_generator();
    create_block(&mut blockchain, txs.take(5).collect());

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    let block = explorer.block(Height(1)).unwrap();
    for tx in &block {
        tx.status().unwrap();
    }
    for (i, tx) in block.iter().enumerate() {
        let raw_tx = tx.message();
        let parsed_tx = raw_tx
            .payload()
            .parse::<CreateWallet>()
            .expect("Unable to parse transaction");
        assert_eq!(parsed_tx.name, format!("Alice #{}", i))
    }

    // Test filtering and other nice stuff.
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    let tx_alice = alice.create_wallet(SERVICE_ID, CreateWallet::new("Alice"));
    let tx_bob = bob.create_wallet(SERVICE_ID, CreateWallet::new("Bob"));
    let tx_transfer = alice.transfer(SERVICE_ID, Transfer::new(bob.public_key(), 2));

    create_block(
        &mut blockchain,
        vec![tx_alice, tx_bob.clone(), tx_transfer.clone()],
    );

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    let block = explorer.block(Height(2)).unwrap();
    let failed_tx_hashes: Vec<_> = block
        .iter()
        .filter(|tx| tx.status().is_err())
        .map(|tx| tx.message().object_hash())
        .collect();
    assert_eq!(
        failed_tx_hashes,
        vec![tx_bob.object_hash(), tx_transfer.object_hash()]
    );

    let create_wallet_positions: Vec<_> = block
        .iter()
        .filter(is_create_wallet)
        .map(|tx| tx.location().position_in_block())
        .collect();
    assert_eq!(create_wallet_positions, vec![0, 1]);
}

#[test]
fn test_block_with_transactions() {
    let mut blockchain = create_blockchain();
    let txs: Vec<_> = tx_generator().take(5).collect();
    create_block(&mut blockchain, txs);

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    let block = explorer.block_with_txs(Height(1)).unwrap();
    assert_eq!(block.len(), 5);
    assert!(!block.is_empty());
    assert!(block[1].status().is_ok());

    let all_transactions_create_wallets = block.iter().all(is_create_wallet);
    assert!(all_transactions_create_wallets);
}

#[test]
#[should_panic(expected = "Index exceeds number of transactions")]
fn test_block_with_transactions_index_overflow() {
    let mut blockchain = create_blockchain();
    let txs: Vec<_> = tx_generator().take(5).collect();
    create_block(&mut blockchain, txs);

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    let block = explorer.block_with_txs(Height(1)).unwrap();
    assert!(block[6].status().is_ok());
}

#[test]
fn test_committed_transaction_roundtrip() {
    let mut blockchain = create_blockchain();
    let tx = tx_generator().next().unwrap();
    create_block(&mut blockchain, vec![tx.clone()]);

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    let tx_copy: &CommittedTransaction = &explorer.block_with_txs(Height(1)).unwrap()[0];
    let json = serde_json::to_value(tx_copy).unwrap();
    let tx_copy: CommittedTransaction = serde_json::from_value(json).unwrap();

    assert_eq!(tx_copy.message(), &tx);
}

#[test]
fn test_transaction_info_roundtrip() {
    let mut blockchain = create_blockchain();
    let tx = tx_generator().next().unwrap();
    blockchain.add_transactions_into_pool(iter::once(tx.clone()));

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    let info: TransactionInfo = explorer.transaction(&tx.object_hash()).unwrap();
    let json = serde_json::to_value(&info).unwrap();
    let info: TransactionInfo = serde_json::from_value(json).unwrap();

    assert_eq!(info.message(), &tx);
}

#[test]
fn test_block_with_transactions_roundtrip() {
    let mut blockchain = create_blockchain();
    let tx = KeyPair::random().create_wallet(SERVICE_ID, CreateWallet::new("Alice"));
    create_block(&mut blockchain, vec![tx]);

    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    let block = explorer.block_with_txs(Height(1)).unwrap();
    let block_json = serde_json::to_value(&block).unwrap();
    let block_copy: BlockWithTransactions = serde_json::from_value(block_json).unwrap();
    assert_eq!(block_copy[0].message(), block[0].message());
}
