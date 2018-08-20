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

//! Examples of usage of a blockchain explorer.

#[macro_use]
extern crate exonum;
#[macro_use]
extern crate serde_json;

use exonum::{
    blockchain::{Blockchain, Schema, Transaction, TransactionError}, crypto, explorer::*,
    helpers::{Height, ValidatorId},
};

use blockchain::{consensus_keys, create_block, create_blockchain, CreateWallet, Transfer};

#[path = "../tests/explorer/blockchain.rs"]
mod blockchain;

/// Creates a transaction for the mempool.
pub fn mempool_transaction() -> Box<Transaction> {
    // Must be deterministic, so we are using consensus keys, which are generated from
    // a passphrase.
    let (pk_alex, key_alex) = consensus_keys();
    CreateWallet::new(&pk_alex, "Alex", &key_alex).into()
}

/// Creates a sample blockchain for the example.
///
/// The blockchain has a single non-genesis block with 3 transactions:
///
/// - A successfully executed transaction
/// - An erroneous transaction
/// - A panicking transaction
///
/// Additionally, a single transaction is placed into the pool.
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

fn main() {
    let blockchain = sample_blockchain();
    let explorer = BlockchainExplorer::new(&blockchain);

    // `BlockInfo` usage
    let block: BlockInfo = explorer.block(Height(1)).unwrap();
    assert_eq!(block.height(), Height(1));
    assert_eq!(block.len(), 3);

    // Iterate over transactions in the block
    for tx in &block {
        println!("{:?}: {:?}", tx.location(), tx.content());
    }

    // `BlockInfo`: JSON presentation
    assert_eq!(
        serde_json::to_value(&block).unwrap(),
        json!({
            // `Block` representation
            "block": block.header(),
            // Array of `Precommit`s
            "precommits": *block.precommits(),
            // Array of transaction hashes
            "txs": *block.transaction_hashes(),
        })
    );

    // `BlockWithTransactions` usage
    let block: BlockWithTransactions = explorer.block_with_txs(Height(1)).unwrap();
    assert_eq!(block.height(), Height(1));
    assert_eq!(block.len(), 3);

    // Iterate over transactions in the block
    for tx in &block {
        println!("{:?}: {:?}", tx.location(), tx.content());
    }
    // Compared to `BlockInfo`, you can access transactions in a block using indexes
    let tx: &CommittedTransaction = &block[1];
    assert_eq!(tx.location().position_in_block(), 1);

    // `CommittedTransaction` usage
    let tx = explorer.block(Height(1)).unwrap().transaction(0).unwrap();
    assert_eq!(tx.location().block_height(), Height(1));
    assert_eq!(tx.location().position_in_block(), 0);

    // It is possible to access transaction content
    let content: &Transaction = tx.content().as_ref();
    println!("{:?}", content);
    // ...and transaction status as well
    let status: Result<(), &TransactionError> = tx.status();
    assert!(status.is_ok());

    // `CommittedTransaction` JSON presentation
    assert_eq!(
        serde_json::to_value(&tx).unwrap(),
        json!({
            // `Transaction` JSON presentation
            "content": tx.content().serialize_field().unwrap(),
            // Position in block
            "location": {
                "block_height": "1",
                "position_in_block": "0",
            },
            // `ListProof` of the transaction inclusion in block
            "location_proof": tx.location_proof(),
            // Execution status
            "status": { "type": "success" },
        })
    );

    // JSON for erroneous transactions
    let erroneous_tx = explorer.block(Height(1)).unwrap().transaction(1).unwrap();
    assert_eq!(
        serde_json::to_value(&erroneous_tx).unwrap(),
        json!({
            "status": {
                "type": "error",
                "code": 1,
                "description": "Not allowed",
            },
            // Other fields...
            "content": erroneous_tx.content().serialize_field().unwrap(),
            "location": erroneous_tx.location(),
            "location_proof": erroneous_tx.location_proof(),
        })
    );

    // JSON for panicking transactions
    let panicked_tx = explorer.block(Height(1)).unwrap().transaction(2).unwrap();
    assert_eq!(
        serde_json::to_value(&panicked_tx).unwrap(),
        json!({
            "status": { "type": "panic", "description": "oops" },
            // Other fields...
            "content": panicked_tx.content().serialize_field().unwrap(),
            "location": panicked_tx.location(),
            "location_proof": panicked_tx.location_proof(),
        })
    );

    // `TransactionInfo` usage
    let hash = mempool_transaction().hash();
    let tx: TransactionInfo = explorer.transaction(&hash).unwrap();
    assert!(tx.is_in_pool());
    println!("{:?}", tx.content());

    // JSON serialization for committed transactions
    let committed_tx: TransactionInfo = explorer.transaction(&block[0].content().hash()).unwrap();
    let tx_ref = committed_tx.as_committed().unwrap();
    assert_eq!(
        serde_json::to_value(&committed_tx).unwrap(),
        json!({
            "type": "committed",
            "content": committed_tx.content().serialize_field().unwrap(),
            "status": { "type": "success" },
            "location": tx_ref.location(),
            "location_proof": tx_ref.location_proof(),
        })
    );

    // JSON serialization for transactions in pool
    let tx_in_pool: TransactionInfo = explorer.transaction(&mempool_transaction().hash()).unwrap();
    assert_eq!(
        serde_json::to_value(&tx_in_pool).unwrap(),
        json!({
            "type": "in-pool",
            "content": tx_in_pool.content().serialize_field().unwrap(),
        })
    );

    // Some more interesting capabilities of the `block` iterator

    // Calculate the total number of transactions in the first 10 blocks
    let tx_count: usize = explorer.blocks(..Height(10)).map(|block| block.len()).sum();
    assert_eq!(tx_count, 3);
    // Determine the number of blocks proposed by a specific validator
    let block_count = explorer
        .blocks(Height(1)..) // skip genesis block
        .filter(|block| block.header().proposer_id() == ValidatorId(0))
        .count();
    assert_eq!(block_count, 1);
}
