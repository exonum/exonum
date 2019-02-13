// Copyright 2019 The Exonum Team
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

#[macro_use]
extern crate exonum_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

use exonum::{
    blockchain::{Schema, TransactionErrorType, TransactionSet, TxLocation},
    crypto::{self, Hash},
    explorer::*,
    helpers::Height,
    messages::{self, Message, RawTransaction, Signed},
};

use crate::blockchain::{
    create_block, create_blockchain, CreateWallet, ExplorerTransactions, Transfer, SERVICE_ID,
};

mod blockchain;

#[test]
fn test_explorer_basics() {
    let mut blockchain = create_blockchain();

    let (pk_alice, key_alice) = crypto::gen_keypair();
    let (pk_bob, key_bob) = crypto::gen_keypair();

    let payload_alice = CreateWallet::new(&pk_alice, "Alice");
    let tx_alice =
        Message::sign_transaction(payload_alice.clone(), SERVICE_ID, pk_alice, &key_alice);

    let payload_bob = CreateWallet::new(&pk_bob, "Bob");
    let tx_bob = Message::sign_transaction(payload_bob.clone(), SERVICE_ID, pk_bob, &key_bob);

    let payload_transfer = Transfer::new(&pk_alice, &pk_bob, 2);
    let tx_transfer =
        Message::sign_transaction(payload_transfer.clone(), SERVICE_ID, pk_alice, &key_alice);

    {
        let explorer = BlockchainExplorer::new(&blockchain);
        assert_eq!(explorer.height(), Height(0));
        let block = explorer.block(Height(0)).unwrap();
        assert_eq!(block.len(), 0);
        assert!(block.transaction(0).is_none());
        assert!(explorer.transaction(&tx_alice.hash()).is_none());
    }

    // Block #1: Alice's transaction.

    create_block(&mut blockchain, vec![tx_alice.clone().into()]);

    {
        let explorer = BlockchainExplorer::new(&blockchain);
        assert_eq!(explorer.height(), Height(1));
        assert!(explorer.block(Height(2)).is_none());

        let block = explorer.block(Height(1)).unwrap();
        assert_eq!(block.len(), 1);
        let tx_info = block.transaction(0).unwrap();
        assert_eq!(*tx_info.location(), TxLocation::new(Height(1), 0));
        assert_eq!(tx_info.status(), Ok(()));
        assert_eq!(tx_info.content().signed_message(), &tx_alice);
        assert_eq!(
            tx_info.content().signed_message().hash(),
            block.transaction_hashes()[0]
        );

        let tx_info = explorer.transaction(&tx_alice.hash()).unwrap();
        assert!(!tx_info.is_in_pool());
        assert!(tx_info.is_committed());
        assert_eq!(tx_info.content().signed_message(), &tx_alice);

        let tx_info = match tx_info {
            TransactionInfo::Committed(info) => info,
            tx => panic!("{:?}", tx),
        };
        assert_eq!(*tx_info.location(), TxLocation::new(Height(1), 0));
        assert_eq!(
            serde_json::to_value(&tx_info).unwrap(),
            json!({
                "content": {
                    "debug": payload_alice,
                    "message": messages::to_hex_string(&tx_alice)
                },
                "location": {
                    "block_height": 1,
                    "position_in_block": 0,
                },
                "location_proof": tx_info.location_proof(), // too complicated to check
                "status": { "type": "success" },
            })
        );
    }

    // Block #2: other transactions.

    create_block(&mut blockchain, vec![tx_bob.clone(), tx_transfer.clone()]);

    let explorer = BlockchainExplorer::new(&blockchain);
    assert_eq!(explorer.height(), Height(2));
    let block = explorer.block(Height(2)).unwrap();
    assert_eq!(block.len(), 2);

    let tx_info = block.transaction(0).unwrap();
    let err = tx_info.status().unwrap_err();
    assert_eq!(err.error_type(), TransactionErrorType::Code(1));
    assert_eq!(err.description(), Some("Not allowed"));
    assert_eq!(
        serde_json::to_value(&tx_info).unwrap(),
        json!({
            "content": {
                    "debug": payload_bob,
                    "message": messages::to_hex_string(&tx_bob)
            },
            "location": {
                "block_height": 2,
                "position_in_block": 0,
            },
            "location_proof": tx_info.location_proof(), // too complicated to check
            "status": {
                "type": "error",
                "code": 1,
                "description": "Not allowed",
            },
        })
    );

    let tx_info = block.transaction(1).unwrap();
    let err = tx_info.status().unwrap_err();
    assert_eq!(err.error_type(), TransactionErrorType::Panic);
    assert_eq!(err.description(), Some("oops"));
    assert_eq!(
        serde_json::to_value(&tx_info).unwrap(),
        json!({
            "content": {
                    "debug": payload_transfer,
                    "message": messages::to_hex_string(&tx_transfer)
            },
            "location": {
                "block_height": 2,
                "position_in_block": 1,
            },
            "location_proof": tx_info.location_proof(), // too complicated to check
            "status": {
                "type": "panic",
                "description": "oops",
            },
        })
    );
}

#[test]
fn test_explorer_pool_transaction() {
    let mut blockchain = create_blockchain();

    let (pk_alice, key_alice) = crypto::gen_keypair();
    let tx_alice = Message::sign_transaction(
        CreateWallet::new(&pk_alice, "Alice"),
        SERVICE_ID,
        pk_alice,
        &key_alice,
    );
    let tx_hash = tx_alice.hash();

    {
        let explorer = BlockchainExplorer::new(&blockchain);
        assert!(explorer.transaction(&tx_hash).is_none());
    }

    let mut fork = blockchain.fork();
    {
        let mut schema = Schema::new(&mut fork);
        schema.add_transaction_into_pool(tx_alice.clone());
    }
    blockchain.merge(fork.into_patch()).unwrap();

    let explorer = BlockchainExplorer::new(&blockchain);
    let tx_info = explorer.transaction(&tx_hash).unwrap();
    assert!(tx_info.is_in_pool());
    assert!(!tx_info.is_committed());
    assert_eq!(tx_info.content().signed_message(), &tx_alice);
}

fn tx_generator() -> Box<dyn Iterator<Item = Signed<RawTransaction>>> {
    Box::new((0..).map(|i| {
        let (pk, key) = crypto::gen_keypair();
        Message::sign_transaction(
            CreateWallet::new(&pk, &format!("Alice #{}", i)),
            SERVICE_ID,
            pk,
            &key,
        )
    }))
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
    assert_eq!(blockchain.last_block().height(), Height(9));

    let explorer = BlockchainExplorer::new(&blockchain);

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

    {
        let explorer = BlockchainExplorer::new(&blockchain);
        let block = explorer.block(Height(1)).unwrap();
        for tx in &block {
            assert_eq!(tx.status(), Ok(()));
        }
        for (i, tx) in block.iter().enumerate() {
            let raw_tx = tx.content().raw_transaction();
            let tx = ExplorerTransactions::tx_from_raw(raw_tx).unwrap();
            match tx {
                ExplorerTransactions::CreateWallet(parsed_tx) => {
                    assert_eq!(parsed_tx.name, format!("Alice #{}", i))
                }
                _ => panic!("Transaction couldn't be parsed."),
            }
        }
    }

    // Test filtering and other nice stuff.

    let (pk_alice, key_alice) = crypto::gen_keypair();
    let (pk_bob, key_bob) = crypto::gen_keypair();
    let tx_alice = Message::sign_transaction(
        CreateWallet::new(&pk_alice, "Alice"),
        SERVICE_ID,
        pk_alice,
        &key_alice,
    );
    let tx_bob = Message::sign_transaction(
        CreateWallet::new(&pk_bob, "Bob"),
        SERVICE_ID,
        pk_bob,
        &key_bob,
    );
    let tx_transfer = Message::sign_transaction(
        Transfer::new(&pk_alice, &pk_bob, 2),
        SERVICE_ID,
        pk_alice,
        &key_alice,
    );
    create_block(
        &mut blockchain,
        vec![tx_alice.clone(), tx_bob.clone(), tx_transfer.clone()],
    );

    let explorer = BlockchainExplorer::new(&blockchain);

    let block = explorer.block(Height(2)).unwrap();
    let failed_tx_hashes: Vec<_> = block
        .iter()
        .filter(|tx| tx.status().is_err())
        .map(|tx| tx.content().signed_message().hash())
        .collect();
    assert_eq!(failed_tx_hashes, vec![tx_bob.hash(), tx_transfer.hash()]);

    let create_wallet_positions: Vec<_> = block
        .iter()
        .filter(|tx| {
            if let ExplorerTransactions::CreateWallet(_) =
                ExplorerTransactions::tx_from_raw(tx.content().raw_transaction()).unwrap()
            {
                true
            } else {
                false
            }
        })
        .map(|tx| tx.location().position_in_block())
        .collect();
    assert_eq!(create_wallet_positions, vec![0, 1]);
}

#[test]
fn test_block_with_transactions() {
    let mut blockchain = create_blockchain();
    let txs: Vec<_> = tx_generator().take(5).collect();
    create_block(&mut blockchain, txs);

    let explorer = BlockchainExplorer::new(&blockchain);
    let block = explorer.block_with_txs(Height(1)).unwrap();
    assert_eq!(block.len(), 5);
    assert!(!block.is_empty());
    assert!(block[1].status().is_ok());

    assert!(block.iter().all(|tx| {
        if let ExplorerTransactions::CreateWallet(_) =
            ExplorerTransactions::tx_from_raw(tx.content().raw_transaction()).unwrap()
        {
            true
        } else {
            false
        }
    }));
}

#[test]
#[should_panic(expected = "Index exceeds number of transactions")]
fn test_block_with_transactions_index_overflow() {
    let mut blockchain = create_blockchain();
    let txs: Vec<_> = tx_generator().take(5).collect();
    create_block(&mut blockchain, txs);

    let explorer = BlockchainExplorer::new(&blockchain);
    let block = explorer.block_with_txs(Height(1)).unwrap();
    assert!(block[6].status().is_ok());
}

#[test]
fn test_committed_transaction_roundtrip() {
    let mut blockchain = create_blockchain();
    let tx = tx_generator().next().unwrap();
    create_block(&mut blockchain, vec![tx.clone()]);

    let explorer = BlockchainExplorer::new(&blockchain);
    let tx_copy: &CommittedTransaction = &explorer.block_with_txs(Height(1)).unwrap()[0];
    let json = serde_json::to_value(tx_copy).unwrap();
    let tx_copy: CommittedTransaction = serde_json::from_value(json).unwrap();

    assert_eq!(tx_copy.content().message(), &tx);
}

#[test]
fn test_transaction_info_roundtrip() {
    let mut blockchain = create_blockchain();
    let tx = tx_generator().next().unwrap();

    let mut fork = blockchain.fork();
    {
        let mut schema = Schema::new(&mut fork);
        schema.add_transaction_into_pool(tx.clone());
    }
    blockchain.merge(fork.into_patch()).unwrap();

    let explorer = BlockchainExplorer::new(&blockchain);
    let info: TransactionInfo = explorer.transaction(&tx.hash()).unwrap();
    let json = serde_json::to_value(&info).unwrap();
    let info: TransactionInfo = serde_json::from_value(json).unwrap();

    assert_eq!(info.content().message(), &tx);
}

#[test]
fn test_block_with_transactions_roundtrip() {
    let mut blockchain = create_blockchain();
    let (pk_alice, key_alice) = crypto::gen_keypair();
    let payload = CreateWallet::new(&pk_alice, "Alice");
    let tx = Message::sign_transaction(payload.clone(), SERVICE_ID, pk_alice, &key_alice);
    create_block(&mut blockchain, vec![tx.clone()]);

    let explorer = BlockchainExplorer::new(&blockchain);
    let block = explorer.block_with_txs(Height(1)).unwrap();
    let block_json = serde_json::to_value(&block).unwrap();
    let block_copy: BlockWithTransactions = serde_json::from_value(block_json).unwrap();
    assert_eq!(
        block_copy[0].content().message(),
        block[0].content().message()
    );
}
