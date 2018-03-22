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

#![cfg_attr(feature = "doctests", allow(unused_imports, dead_code))]

use futures::sync::mpsc;
use serde_json;

use blockchain::{ExecutionError, ExecutionResult, Service, TransactionSet};
use crypto::{self, CryptoHash, PublicKey, SecretKey};
use encoding::Error as EncodingError;
use messages::{Message, RawTransaction, ServiceMessage};
use node::ApiSender;
use storage::{Fork, MemoryDB, Snapshot};
use super::*;

// // // Transactions // // //

transactions! {
    Transactions {
        const SERVICE_ID = 10_000;

        struct CreateWallet {
            pubkey: &PublicKey,
            name: &str,
        }

        struct Transfer {
            from: &PublicKey,
            to: &PublicKey,
            amount: u64,
        }
    }
}

impl Transaction for CreateWallet {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, _: &mut Fork) -> ExecutionResult {
        if self.name().starts_with("Al") {
            Ok(())
        } else {
            Err(ExecutionError::with_description(
                1,
                "Not allowed".to_string(),
            ))
        }
    }
}

impl Transaction for Transfer {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, _: &mut Fork) -> ExecutionResult {
        panic!("oops")
    }
}

// // // Service // // //

struct MyService;

impl Service for MyService {
    fn service_id(&self) -> u16 {
        10_000
    }

    fn service_name(&self) -> &str {
        "my-service"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
        Transactions::tx_from_raw(raw).map(Transactions::into)
    }
}

fn consensus_keys() -> (PublicKey, SecretKey) {
    const SEED_PHRASE: &[u8] = b"correct horse battery staple";
    let seed = crypto::Seed::from_slice(crypto::hash(SEED_PHRASE).as_ref()).unwrap();
    crypto::gen_keypair_from_seed(&seed)
}

fn create_blockchain() -> Blockchain {
    use blockchain::{GenesisConfig, ValidatorKeys};

    let (consensus_key, _) = consensus_keys();
    let service_keys = crypto::gen_keypair();

    let api_channel = mpsc::channel(10);
    let mut blockchain = Blockchain::new(
        MemoryDB::new(),
        vec![MyService.into()],
        service_keys.0,
        service_keys.1,
        ApiSender(api_channel.0),
    );

    let keys = ValidatorKeys {
        consensus_key,
        service_key: service_keys.0,
    };
    blockchain
        .initialize(GenesisConfig::new(vec![keys].into_iter()))
        .unwrap();
    blockchain
}

// Simplified compared to real life / testkit, but we don't need to test *everything*
// here.
fn create_block(blockchain: &mut Blockchain, transactions: Vec<Box<Transaction>>) {
    use std::time::SystemTime;
    use helpers::{Round, ValidatorId};
    use messages::Propose;

    let tx_hashes: Vec<_> = transactions.iter().map(|tx| tx.hash()).collect();
    let height = blockchain.last_block().height().next();

    let mut fork = blockchain.fork();
    {
        let mut schema = Schema::new(&mut fork);
        for tx in transactions {
            schema.add_transaction_into_pool(tx.raw().clone())
        }
    }
    blockchain.merge(fork.into_patch()).unwrap();

    let (block_hash, patch) = blockchain.create_patch(ValidatorId(0), height, &tx_hashes);
    let (_, consensus_secret_key) = consensus_keys();

    let propose = Propose::new(
        ValidatorId(0),
        height,
        Round::first(),
        &blockchain.last_hash(),
        &tx_hashes,
        &consensus_secret_key,
    );
    let precommit = Precommit::new(
        ValidatorId(0),
        propose.height(),
        propose.round(),
        &propose.hash(),
        &block_hash,
        SystemTime::now().into(),
        &consensus_secret_key,
    );

    blockchain
        .commit(&patch, block_hash, [precommit].into_iter())
        .unwrap();
}

/// Creates a transaction for the mempool.
#[cfg(feature = "doctests")]
pub fn mempool_transaction() -> Box<Transaction> {
    let (pk_alex, key_alex) = consensus_keys(); // Must be deterministic!
    CreateWallet::new(&pk_alex, "Alex", &key_alex).into()
}

/// Creates a sample blockchain for doc tests.
#[cfg(feature = "doctests")]
pub fn sample_blockchain() -> Blockchain {
    let mut blockchain = create_blockchain();
    let (pk_alice, key_alice) = crypto::gen_keypair();
    let (pk_bob, key_bob) = crypto::gen_keypair();
    let tx_alice = CreateWallet::new(&pk_alice, "Alice", &key_alice);
    let tx_bob = CreateWallet::new(&pk_bob, "Bob", &key_bob);
    let tx_transfer = Transfer::new(&pk_alice, &pk_bob, 100, &key_alice);

    create_block(
        &mut blockchain,
        vec![tx_alice.into(), tx_bob.into(), tx_transfer.into()],
    );

    let mut fork = blockchain.fork();
    {
        let mut schema = Schema::new(&mut fork);
        schema.add_transaction_into_pool(mempool_transaction().raw().clone());
    }
    blockchain.merge(fork.into_patch()).unwrap();

    blockchain
}

#[test]
fn test_explorer_basics() {
    let mut blockchain = create_blockchain();

    let (pk_alice, key_alice) = crypto::gen_keypair();
    let (pk_bob, key_bob) = crypto::gen_keypair();

    let tx_alice = CreateWallet::new(&pk_alice, "Alice", &key_alice);
    let tx_bob = CreateWallet::new(&pk_bob, "Bob", &key_bob);
    let tx_transfer = Transfer::new(&pk_alice, &pk_bob, 100, &key_alice);

    let explorer = BlockchainExplorer::new(blockchain.clone());
    let block = explorer.block(Height(0)).unwrap();
    assert_eq!(block.len(), 0);
    assert!(block.transaction(0).is_none());
    assert!(explorer.transaction(&tx_alice.hash()).is_none());

    // Block #1: Alice's transaction.

    create_block(&mut blockchain, vec![tx_alice.clone().into()]);
    assert!(explorer.block(Height(2)).is_none());
    let block = explorer.block(Height(1)).unwrap();
    assert_eq!(block.len(), 1);
    let tx_info = block.transaction(0).unwrap();
    assert_eq!(*tx_info.location(), TxLocation::new(Height(1), 0));
    assert_eq!(tx_info.status(), Ok(()));
    assert_eq!(tx_info.content().raw(), tx_alice.raw());
    assert_eq!(tx_info.content().hash(), block.transaction_hashes()[0]);

    let tx_info = explorer.transaction(&tx_alice.hash()).unwrap();
    assert!(!tx_info.is_in_pool());
    assert!(tx_info.is_committed());
    assert_eq!(tx_info.content().raw(), tx_alice.raw());

    let tx_info = match tx_info {
        TransactionInfo::Committed(info) => info,
        tx => panic!("{:?}", tx),
    };
    assert_eq!(*tx_info.location(), TxLocation::new(Height(1), 0));
    assert_eq!(
        serde_json::to_value(&tx_info).unwrap(),
        json!({
            "content": tx_alice,
            "location": {
                "block_height": "1",
                "position_in_block": "0",
            },
            "location_proof": tx_info.location_proof(), // too complicated to check
            "status": { "type": "success" },
        })
    );

    // Block #2: other transactions.

    create_block(
        &mut blockchain,
        vec![tx_bob.clone().into(), tx_transfer.clone().into()],
    );
    let block = explorer.block(Height(2)).unwrap();
    assert_eq!(block.len(), 2);

    let tx_info = block.transaction(0).unwrap();
    let err = tx_info.status().unwrap_err();
    assert_eq!(err.error_type(), TransactionErrorType::Code(1));
    assert_eq!(err.description(), Some("Not allowed"));
    assert_eq!(
        serde_json::to_value(&tx_info).unwrap(),
        json!({
            "content": tx_bob,
            "location": {
                "block_height": "2",
                "position_in_block": "0",
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
            "content": tx_transfer,
            "location": {
                "block_height": "2",
                "position_in_block": "1",
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
    let tx_alice = CreateWallet::new(&pk_alice, "Alice", &key_alice);
    let tx_hash = tx_alice.hash();

    let explorer = BlockchainExplorer::new(blockchain.clone());
    assert!(explorer.transaction(&tx_hash).is_none());

    let mut fork = blockchain.fork();
    {
        let mut schema = Schema::new(&mut fork);
        schema.add_transaction_into_pool(tx_alice.raw().clone());
    }
    blockchain.merge(fork.into_patch()).unwrap();

    let tx_info = explorer.transaction(&tx_hash).unwrap();
    assert!(tx_info.is_in_pool());
    assert!(!tx_info.is_committed());
    assert_eq!(tx_info.content().raw(), tx_alice.raw());
}

fn tx_generator() -> Box<Iterator<Item = Box<Transaction>>> {
    Box::new((0..).map(|i| {
        let (pk, key) = crypto::gen_keypair();
        let tx = CreateWallet::new(&pk, &format!("Alice #{}", i), &key);
        Box::new(tx) as Box<Transaction>
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

    let explorer = BlockchainExplorer::new(blockchain);

    let mut count = 0;
    for (i, block) in explorer.blocks(..).enumerate() {
        assert_eq!(block.block().height(), Height(i as u64));
        count += 1;
    }
    assert_eq!(count, 10);

    let tx_count: usize = explorer.blocks(..Height(1_000)).map(|bl| bl.len()).sum();
    assert_eq!(tx_count, 12);

    let count = explorer.blocks(..).filter(BlockInfo::is_empty).count();
    assert_eq!(count, 6);

    let heights: Vec<_> = explorer
        .blocks(..)
        .rev()
        .filter(|bl| !bl.is_empty())
        .map(|bl| bl.block().height())
        .collect();
    assert_eq!(heights, vec![Height(9), Height(5), Height(4), Height(2)]);

    let block = explorer
        .blocks(..)
        .filter(|bl| !bl.is_empty())
        .nth(2)
        .unwrap();
    assert_eq!(block.block().height(), Height(5));

    let mut iter = explorer.blocks(..).filter(|bl| !bl.is_empty()).rev();
    let _ = iter.by_ref().nth(1);
    let heights: Vec<_> = iter.by_ref().map(|bl| bl.block().height()).collect();
    assert_eq!(heights, vec![Height(4), Height(2)]);
    assert!(iter.next().is_none());

    let heights: Vec<_> = explorer
        .blocks(..Height(9))
        .filter(|bl| !bl.is_empty())
        .rev()
        .map(|bl| bl.block().height())
        .collect();
    assert_eq!(heights, vec![Height(5), Height(4), Height(2)]);

    let heights: Vec<_> = explorer
        .blocks(..Height(5))
        .filter(|bl| !bl.is_empty())
        .map(|bl| bl.block().height())
        .collect();
    assert_eq!(heights, vec![Height(2), Height(4)]);

    let heights: Vec<_> = explorer
        .blocks(Height(0)..Height(2))
        .filter(|bl| !bl.is_empty())
        .map(|bl| bl.block().height())
        .collect();
    assert_eq!(heights, vec![]);

    let heights: Vec<_> = explorer
        .blocks(..Height(2))
        .map(|bl| bl.block().height())
        .collect();
    assert_eq!(heights, vec![Height(0), Height(1)]);

    let heights: Vec<_> = explorer
        .blocks(..Height(1))
        .map(|bl| bl.block().height())
        .collect();
    assert_eq!(heights, vec![Height(0)]);

    let heights: Vec<_> = explorer
        .blocks(..Height(0))
        .map(|bl| bl.block().height())
        .collect();
    assert_eq!(heights, vec![]);

    let heights: Vec<_> = explorer
        .blocks(Height(0)..Height(1))
        .map(|bl| bl.block().height())
        .collect();
    assert_eq!(heights, vec![Height(0)]);

    let mut iter = explorer.blocks(..).map(|bl| bl.block().clone());
    assert_eq!(iter.by_ref().nth(3).unwrap().height(), Height(3));
    assert_eq!(
        iter.nth(2).unwrap().height(),
        Height(6)
    );
}

#[test]
fn test_transaction_iterator() {
    let mut blockchain = create_blockchain();
    let txs = tx_generator();
    create_block(&mut blockchain, txs.take(5).collect());

    let explorer = BlockchainExplorer::new(blockchain.clone());
    let block = explorer.block(Height(1)).unwrap();
    for tx in &block {
        assert_eq!(tx.status(), Ok(()));
    }
    for (i, tx) in block.iter().enumerate() {
        let parsed_tx = CreateWallet::from_raw(tx.content().raw().clone()).unwrap();
        assert_eq!(parsed_tx.name(), &format!("Alice #{}", i));
    }

    // Test filtering and other nice stuff.

    let (pk_alice, key_alice) = crypto::gen_keypair();
    let (pk_bob, key_bob) = crypto::gen_keypair();
    let tx_alice = CreateWallet::new(&pk_alice, "Alice", &key_alice);
    let tx_bob = CreateWallet::new(&pk_bob, "Bob", &key_bob);
    let tx_transfer = Transfer::new(&pk_alice, &pk_bob, 100, &key_alice);
    create_block(
        &mut blockchain,
        vec![tx_alice.clone().into(), tx_bob.clone().into(), tx_transfer.clone().into()],
    );

    let block = explorer.block(Height(2)).unwrap();
    let failed_tx_hashes: Vec<_> = block
        .iter()
        .filter(|tx| tx.status.is_err())
        .map(|tx| tx.content().hash())
        .collect();
    assert_eq!(failed_tx_hashes, vec![tx_bob.hash(), tx_transfer.hash()]);

    let create_wallet_positions: Vec<_> = block
        .iter()
        .filter(|tx| {
            tx.content().raw().message_type() == CreateWallet::MESSAGE_ID
        })
        .map(|tx| tx.location().position_in_block())
        .collect();
    assert_eq!(create_wallet_positions, vec![0, 1]);
}

#[test]
fn test_block_with_transactions() {
    let mut blockchain = create_blockchain();
    let txs: Vec<_> = tx_generator().take(5).collect();
    let tx_hashes: Vec<_> = txs.iter().map(|tx| tx.hash()).collect();
    create_block(&mut blockchain, txs);

    let explorer = BlockchainExplorer::new(blockchain);
    let block = explorer.block_with_txs(Height(1)).unwrap();
    assert_eq!(block.len(), 5);
    assert!(!block.is_empty());
    assert!(block[1].status().is_ok());
    assert_eq!(block[&tx_hashes[3]].location().position_in_block(), 3);

    assert!(block.iter().all(|tx| tx.content().raw().message_type() == CreateWallet::MESSAGE_ID));
}

#[test]
#[should_panic(expected = "Index exceeds number of transactions")]
fn test_block_with_transactions_index_overflow() {
    let mut blockchain = create_blockchain();
    let txs: Vec<_> = tx_generator().take(5).collect();
    create_block(&mut blockchain, txs);

    let explorer = BlockchainExplorer::new(blockchain);
    let block = explorer.block_with_txs(Height(1)).unwrap();
    assert!(block[6].status().is_ok());
}

#[test]
#[should_panic(expected = "not in block")]
fn test_block_with_transactions_nonexisting_hash() {
    let mut blockchain = create_blockchain();
    let txs: Vec<_> = tx_generator().take(5).collect();
    create_block(&mut blockchain, txs);

    let explorer = BlockchainExplorer::new(blockchain);
    let block = explorer.block_with_txs(Height(1)).unwrap();
    assert!(block[&Hash::zero()].status().is_ok());
}
