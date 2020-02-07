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

//! Examples of usage of a blockchain explorer.

use exonum::{
    blockchain::{BlockchainMut, CallInBlock, ProposerId},
    crypto::KeyPair,
    helpers::{Height, ValidatorId},
    merkledb::{MapProof, ObjectHash},
    messages::{AnyTx, Verified},
    runtime::ErrorKind as ExecutionErrorKind,
};
use serde_json::json;

use std::{collections::BTreeMap, iter};

use exonum_explorer::*;

#[path = "../tests/blockchain/mod.rs"]
mod blockchain;

use crate::blockchain::{
    consensus_keys, create_block, create_blockchain, CreateWallet, ExplorerTransactions as _,
    Transfer, SERVICE_ID,
};

/// Creates a transaction for the mempool.
pub fn mempool_transaction() -> Verified<AnyTx> {
    // Must be deterministic, so we are using consensus keys, which are generated from
    // a passphrase.
    consensus_keys().create_wallet(SERVICE_ID, CreateWallet::new("Alex"))
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
pub fn sample_blockchain() -> BlockchainMut {
    let mut blockchain = create_blockchain();
    let alice = KeyPair::random();
    let bob = KeyPair::random();

    let tx_alice = alice.create_wallet(SERVICE_ID, CreateWallet::new("Alice"));
    let tx_bob = bob.create_wallet(SERVICE_ID, CreateWallet::new("Bob"));
    let tx_transfer = alice.transfer(SERVICE_ID, Transfer::new(bob.public_key(), 100));
    create_block(&mut blockchain, vec![tx_alice, tx_bob, tx_transfer]);

    blockchain.add_transactions_into_pool(iter::once(mempool_transaction()));
    blockchain
}

fn main() {
    let blockchain = sample_blockchain();
    let snapshot = blockchain.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());

    // `BlockInfo` usage
    let block: BlockInfo<'_> = explorer.block(Height(1)).unwrap();
    assert_eq!(block.height(), Height(1));
    assert_eq!(block.len(), 3);

    // Iterate over transactions in the block
    for tx in &block {
        println!("{:?}: {:?}", tx.location(), tx.message());
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
        println!("{:?}: {:?}", tx.location(), tx.message());
    }
    // Compared to `BlockInfo`, you can access transactions in a block using indexes
    let tx: &CommittedTransaction = &block[1];
    assert_eq!(tx.location().position_in_block(), 1);

    // `CommittedTransaction` usage
    let tx = explorer.block(Height(1)).unwrap().transaction(0).unwrap();
    assert_eq!(tx.location().block_height(), Height(1));
    assert_eq!(tx.location().position_in_block(), 0);

    // It is possible to access transaction message
    let message = tx.message();
    println!("{:?}", message);
    // ...and transaction status as well
    let status = tx.status();
    assert!(status.is_ok());

    // `CommittedTransaction` JSON presentation
    assert_eq!(
        serde_json::to_value(&tx).unwrap(),
        json!({
            // `Transaction` JSON presentation
            "message": serde_json::to_value(tx.message()).unwrap(),
            // Position in block
            "location": {
                "block_height": 1,
                "position_in_block": 0,
            },
            // `ListProof` of the transaction inclusion in block
            "location_proof": tx.location_proof(),
            // Execution status
            "status": { "type": "success" },
            "time": tx.time(),
        })
    );

    // JSON for erroneous transactions
    let block_info = explorer.block(Height(1)).unwrap();
    let erroneous_tx = block_info.transaction(1).unwrap();
    assert_eq!(
        serde_json::to_value(&erroneous_tx).unwrap(),
        json!({
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
            // Other fields...
            "message": serde_json::to_value(erroneous_tx.message()).unwrap(),
            "location": erroneous_tx.location(),
            "location_proof": erroneous_tx.location_proof(),
            "time": erroneous_tx.time(),
        })
    );

    // `BlockWithTransactions` contains errors that have occurred during block execution.
    for (i, error) in block.errors.iter().enumerate() {
        println!("Error #{}: {}", i + 1, error);
    }
    // In this block, two errors correspond to 2nd and 3rd transactions. Originally, errors
    // are stored in a `Vec` for serialization reasons, but they can be converted
    // into a `BTreeMap` with a builtin method.
    let errors: BTreeMap<_, _> = block.error_map();
    assert_eq!(errors.len(), 2);
    assert_eq!(
        errors[&CallInBlock::transaction(1)].description(),
        "Not allowed!"
    );
    assert_eq!(
        errors[&CallInBlock::transaction(2)].kind(),
        ExecutionErrorKind::Unexpected
    );

    // It is possible to extract a proof of a transaction error using `BlockInfo`. The proof is tied
    // to the `error_hash` mentioned in the block header.
    let proof: MapProof<_, _> = block_info.error_proof(CallInBlock::transaction(1));
    let proof = proof
        .check_against_hash(block_info.header().error_hash)
        .unwrap();
    let (_, error) = proof.entries().next().unwrap();
    assert_eq!(error.description(), "Not allowed!");

    // JSON for a transaction with a panic in service code (termed "unexpected errors"
    // for compatibility with other runtimes).
    let panicked_tx = explorer.block(Height(1)).unwrap().transaction(2).unwrap();
    assert_eq!(
        serde_json::to_value(&panicked_tx).unwrap(),
        json!({
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
            // Other fields...
            "message": serde_json::to_value(panicked_tx.message()).unwrap(),
            "location": panicked_tx.location(),
            "location_proof": panicked_tx.location_proof(),
            "time": panicked_tx.time(),
        })
    );

    // `TransactionInfo` usage
    let hash = mempool_transaction().object_hash();
    let tx: TransactionInfo = explorer.transaction(&hash).unwrap();
    assert!(tx.is_in_pool());
    println!("{:?}", tx.message());

    // JSON serialization for committed transactions
    let committed_tx: TransactionInfo = explorer
        .transaction(&block[0].message().object_hash())
        .unwrap();
    let tx_ref = committed_tx.as_committed().unwrap();
    assert_eq!(
        serde_json::to_value(&committed_tx).unwrap(),
        json!({
            "type": "committed",
            "message": serde_json::to_value(committed_tx.message()).unwrap(),
            "status": { "type": "success" },
            "location": tx_ref.location(),
            "location_proof": tx_ref.location_proof(),
            "time": tx_ref.time(),
        })
    );

    // JSON serialization for transactions in pool
    let tx_in_pool: TransactionInfo = explorer
        .transaction(&mempool_transaction().object_hash())
        .unwrap();
    assert_eq!(
        serde_json::to_value(&tx_in_pool).unwrap(),
        json!({
            "type": "in_pool",
            "message": serde_json::to_value(tx_in_pool.message()).unwrap(),
        })
    );

    // Some more interesting capabilities of the `block` iterator

    // Calculate the total number of transactions in the first 10 blocks
    let tx_count: usize = explorer.blocks(..Height(10)).map(|block| block.len()).sum();
    assert_eq!(tx_count, 3);
    // Determine the number of blocks proposed by a specific validator
    let block_count = explorer
        .blocks(Height(1)..) // skip genesis block
        .filter(|block| block.header().get_header::<ProposerId>().unwrap() == Some(ValidatorId(0)))
        .count();
    assert_eq!(block_count, 1);
}
