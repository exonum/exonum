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

//! This example shows how to use MerkleDB in a semi-realistic blockchain setup.
//! The app maintains wallets identified by an Ed25519 public key. Wallet information
//! can be modified by the transactions that transfer value between two wallets.
//! Transactions are grouped into blocks. Wallets, transactions and blocks are stored in MerkleDB.

use exonum_crypto::{Hash, KeyPair, PublicKey};
use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
use serde_derive::{Deserialize, Serialize};

use exonum_merkledb::{
    access::{Access, FromAccess, RawAccessMut},
    Database, Fork, Group, ListIndex, MapIndex, ObjectHash, ProofListIndex, ProofMapIndex,
    TemporaryDB,
};

/// Wallet data type that will be stored in the database.
///
/// MerkleDB does not dictate the serialization format, instead requiring that stored values
/// implement the `BinaryValue` trait (and, for some collection types, `ObjectHash`).
/// Here, we derive these traits and use `bincode` serialization format.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
struct Wallet {
    /// Incoming wallet flow.
    incoming: u32,
    /// Outgoing wallet flow. We assume that the wallet owner has an unlimited credit line;
    /// in other words, `outgoing` may exceed `incoming`.
    outgoing: u32,
    /// Hash of the Merkelized list containing all transaction hashes related to the wallet.
    ///
    /// By including this hash into a `Wallet`, we bind the wallet history to the aggregated
    /// database state, so that a lightweight cryptographic proof regarding history can be
    /// retrieved from the database. In a distributed networks (e.g., a blockchain),
    /// including the wallet history into aggregation also guarantees its agreement among
    /// all nodes in the network.
    history_root: Hash,
}

/// Semi-realistic transfer transaction between two wallets.
///
/// Like with `Wallet`, we derive `BinaryValue` and `ObjectHash` to be able to store `Transaction`s
/// in the database.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
struct Transaction {
    sender: PublicKey,
    receiver: PublicKey,
    amount: u32,
}

/// Block of `Transaction`s.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
struct Block {
    /// Hash link to the previous block.
    prev_block: Hash,
    transactions: Vec<Transaction>,
}

impl Transaction {
    /// Emulates transaction execution within a smart contract. The transaction decreases the balance
    /// of the `sender` and increases the balance of the `receiver`. Both `sender` and `receiver`
    /// wallets also get their history updated.
    fn execute(&self, fork: &Fork) {
        let tx_hash = self.object_hash();

        let mut schema = Schema::new(fork);
        schema.transactions.put(&self.object_hash(), *self);

        let mut owner_wallet = schema.wallets.get(&self.sender).unwrap_or_default();
        owner_wallet.outgoing += self.amount;
        owner_wallet.history_root = schema.add_transaction_to_history(&self.sender, tx_hash);
        schema.wallets.put(&self.sender, owner_wallet);

        let mut receiver_wallet = schema.wallets.get(&self.receiver).unwrap_or_default();
        receiver_wallet.incoming += self.amount;
        receiver_wallet.history_root = schema.add_transaction_to_history(&self.receiver, tx_hash);
        schema.wallets.put(&self.receiver, receiver_wallet);
    }
}

/// Data schema used in the example.
///
/// `FromAccess` trait allows to create the schema from an `Access` to the database. Here,
/// we derive this trait, which enables a straightforward declarative description of the
/// schema components (singular indexes and index groups).
#[derive(FromAccess)]
struct Schema<T: Access> {
    /// Map of transaction hashes to transactions.
    pub transactions: MapIndex<T::Base, Hash, Transaction>,
    /// List of accepted blocks of transactions.
    pub blocks: ListIndex<T::Base, Block>,
    /// Map of the owner's public key to the corresponding wallet information.
    /// Note the use of `ProofMapIndex`; the `Proof*` naming signals that the index is Merkelized.
    pub wallets: ProofMapIndex<T::Base, PublicKey, Wallet>,
    /// Group containing wallet histories. The group is keyed by the public key corresponding
    /// to the wallet.
    pub wallet_history: Group<T, PublicKey, ProofListIndex<T::Base, Hash>>,
}

impl<T: Access> Schema<T> {
    fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

// The `RawAccessMut` specialization allows to access index methods mutating the database.
impl<T: Access> Schema<T>
where
    T::Base: RawAccessMut,
{
    /// Adds a transaction hash to the history of a specific wallet and returns the updated
    /// history hash.
    fn add_transaction_to_history(&self, owner: &PublicKey, tx_hash: Hash) -> Hash {
        let mut history = self.wallet_history.get(owner);
        history.push(tx_hash);
        history.object_hash()
    }
}

impl Block {
    /// Executes a block of `Transaction`s and records the changes in the provided database.
    fn execute(self, db: &TemporaryDB) {
        // A `Fork` accumulates changes to the database in RAM.
        let fork = db.fork();
        for transaction in &self.transactions {
            transaction.execute(&fork);
        }
        Schema::new(&fork).blocks.push(self);
        // A fork can be converted to a `Patch` and merged into the database atomically.
        db.merge(fork.into_patch()).unwrap();
    }
}

fn main() {
    // Creates a database instance in the temporary directory. It will be
    // removed when the DB object gets out of scope.
    let db = TemporaryDB::new();

    // Creates an empty genesis block.
    let genesis = Block {
        prev_block: Hash::zero(),
        transactions: vec![],
    };
    let genesis_hash = genesis.object_hash();
    genesis.execute(&db);

    // Create random user keys.
    let alice = KeyPair::random().public_key();
    let bob = KeyPair::random().public_key();

    // Create a transaction that transfers money from Alice to Bob.
    let transaction = Transaction {
        sender: alice,
        receiver: bob,
        amount: 100,
    };
    let tx_hash = transaction.object_hash();
    let block = Block {
        prev_block: genesis_hash,
        transactions: vec![transaction],
    };

    // Execute a block to persist our state of the blockchain.
    block.execute(&db);

    // Get a snapshot of the current database state.
    let snapshot = db.snapshot();
    let schema = Schema::new(&snapshot);

    // Check that Alice and Bob have the expected balances.
    let alice_wallet = schema.wallets.get(&alice).unwrap();
    let bob_wallet = schema.wallets.get(&bob).unwrap();

    assert_eq!(alice_wallet.outgoing, 100);
    assert_eq!(bob_wallet.incoming, 100);

    // Get and check a proof of existence of Alice's wallet in the blockchain.
    let proof = schema.wallets.get_proof(alice);
    let checked_proof = proof.check().unwrap();
    assert_eq!(
        checked_proof.entries().collect::<Vec<_>>(),
        vec![(&alice, &alice_wallet)]
    );

    // Check that the transaction is recorded in wallet history.
    let history = schema.wallet_history.get(&alice);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0), Some(tx_hash));
    let history = schema.wallet_history.get(&bob);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0), Some(tx_hash));

    // Get and check the Bob's history proof.
    let proof = history.get_range_proof(..);
    let checked_proof = proof.check_against_hash(bob_wallet.history_root).unwrap();
    assert_eq!(checked_proof.entries(), [(0, tx_hash)]);
}
