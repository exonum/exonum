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

use futures::sync::mpsc;
use serde_json;

use blockchain::{ExecutionError, ExecutionResult, Service, TransactionSet};
use crypto::{self, CryptoHash, PublicKey, SecretKey};
use encoding::Error as EncodingError;
use messages::{Message, RawTransaction};
use node::ApiSender;
use storage::{Fork, MemoryDB, Snapshot};
use super::*;

lazy_static! {
    static ref KEYS: (PublicKey, SecretKey) = crypto::gen_keypair();
}

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
                format!("{} is not allowed", self.name()),
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

fn create_blockchain() -> Blockchain {
    use blockchain::{GenesisConfig, ValidatorKeys};

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
        consensus_key: KEYS.0,
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
    use std::collections::BTreeMap;
    use std::time::SystemTime;
    use helpers::{Round, ValidatorId};
    use messages::Propose;

    let tx_hashes: Vec<_> = transactions.iter().map(|tx| tx.hash()).collect();
    let pool: BTreeMap<_, _> = transactions.into_iter().map(|tx| (tx.hash(), tx)).collect();
    let height = blockchain.last_block().height().next();

    let (block_hash, patch) = blockchain.create_patch(ValidatorId(0), height, &tx_hashes, &pool);

    let propose = Propose::new(
        ValidatorId(0),
        height,
        Round::first(),
        &blockchain.last_hash(),
        &tx_hashes,
        &KEYS.1,
    );
    let precommit = Precommit::new(
        ValidatorId(0),
        propose.height(),
        propose.round(),
        &propose.hash(),
        &block_hash,
        SystemTime::now(),
        &KEYS.1,
    );

    blockchain
        .commit(&patch, block_hash, [precommit].into_iter())
        .unwrap();
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
    assert_eq!(*tx_info.status(), Ok(()));
    assert_eq!(tx_info.content().raw(), tx_alice.raw());

    let tx_info = explorer.transaction(&tx_alice.hash()).unwrap();
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
    let err = tx_info.status().as_ref().unwrap_err();
    assert_eq!(err.error_type(), TransactionErrorType::Code(1));
    assert_eq!(err.description(), Some("Bob is not allowed"));
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
                "description": "Bob is not allowed",
            },
        })
    );

    let tx_info = block.transaction(1).unwrap();
    let err = tx_info.status().as_ref().unwrap_err();
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
fn test_explorer_block_iter() {
    let mut blockchain = create_blockchain();
    let mut tx_gen = (0..).map(|i| {
        let (pk, key) = crypto::gen_keypair();
        let tx = CreateWallet::new(&pk, &format!("Alice #{}", i), &key);
        Box::new(tx) as Box<Transaction>
    });
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
    for (i, block) in explorer.blocks_rev(false).enumerate() {
        assert_eq!(block.height(), Height(9 - i as u64));
        count += 1;
    }
    assert_eq!(count, 10);

    let heights: Vec<_> = explorer.blocks_rev(true).map(|bl| bl.height()).collect();
    assert_eq!(heights, vec![Height(9), Height(5), Height(4), Height(2)]);

    let block = explorer.blocks_rev(true).nth(2).unwrap();
    assert_eq!(block.height(), Height(4));

    let mut iter = explorer.blocks_rev(true);
    let _ = iter.by_ref().nth(1);
    let heights: Vec<_> = iter.by_ref().map(|bl| bl.height()).collect();
    assert_eq!(heights, vec![Height(4), Height(2)]);
    assert!(iter.next().is_none());

    let heights: Vec<_> = explorer
        .blocks_rev(true)
        .skip_to(Height(8))
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![Height(5), Height(4), Height(2)]);
    let heights: Vec<_> = explorer
        .blocks_rev(true)
        .skip_to(Height(4))
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![Height(4), Height(2)]);
    let heights: Vec<_> = explorer
        .blocks_rev(true)
        .skip_to(Height(1))
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![]);
    let heights: Vec<_> = explorer
        .blocks_rev(false)
        .skip_to(Height(1))
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![Height(1), Height(0)]);
    let heights: Vec<_> = explorer
        .blocks_rev(false)
        .skip_to(Height(0))
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![Height(0)]);

    let heights: Vec<_> = explorer
        .blocks_rev(false)
        .skip_to(Height(0))
        .map(|bl| bl.height())
        .collect();
    assert_eq!(heights, vec![Height(0)]);

    let mut iter = explorer.blocks_rev(true);
    assert_eq!(iter.by_ref().skip_to(Height(6)).next().unwrap().height(), Height(5));
    assert_eq!(iter.skip_to(Height(3)).next().unwrap().height(), Height(2));
    assert!(iter.skip_to(Height(1000)).next().is_none());
}

#[test]
fn test_transaction_iterator() {
    use messages::ServiceMessage;

    let mut blockchain = create_blockchain();
    let mut tx_gen = (0..).map(|i| {
        let (pk, key) = crypto::gen_keypair();
        let tx = CreateWallet::new(&pk, &format!("Alice #{}", i), &key);
        Box::new(tx) as Box<Transaction>
    });
    let txs = tx_gen.by_ref();
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
