use serde_derive::{Deserialize, Serialize};

use std::{borrow::Cow, convert::AsRef};

use exonum_crypto::{Hash, PublicKey};
use exonum_merkledb::{
    impl_object_hash_for_binary_value, BinaryValue, Database, Fork, ListIndex, MapIndex,
    ObjectAccess, ObjectHash, ProofListIndex, ProofMapIndex, RefMut, TemporaryDB,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
struct Wallet {
    incoming: u32,
    outgoing: u32,
    history_root: Hash,
}

impl BinaryValue for Wallet {
    fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        bincode::deserialize(bytes.as_ref()).map_err(From::from)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Transaction {
    sender: PublicKey,
    receiver: PublicKey,
    amount: u32,
}

impl BinaryValue for Transaction {
    fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        bincode::deserialize(bytes.as_ref()).map_err(From::from)
    }
}

impl_object_hash_for_binary_value! { Transaction, Block, Wallet }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Block {
    prev_block: Hash,
    transactions: Vec<Transaction>,
}

impl BinaryValue for Block {
    fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        bincode::deserialize(bytes.as_ref()).map_err(From::from)
    }
}

impl Transaction {
    fn execute(&self, fork: &Fork) {
        let tx_hash = self.object_hash();

        let schema = Schema::new(fork);
        schema.transactions().put(&self.object_hash(), *self);

        let mut owner_wallet = schema.wallets().get(&self.sender).unwrap_or_default();
        owner_wallet.outgoing += self.amount;
        owner_wallet.history_root = schema.add_transaction_to_history(&self.sender, tx_hash);
        schema.wallets().put(&self.sender, owner_wallet);

        let mut receiver_wallet = schema.wallets().get(&self.receiver).unwrap_or_default();
        receiver_wallet.incoming += self.amount;
        receiver_wallet.history_root = schema.add_transaction_to_history(&self.receiver, tx_hash);
        schema.wallets().put(&self.receiver, receiver_wallet);
    }
}

struct Schema<T: ObjectAccess>(T);

impl<T: ObjectAccess> Schema<T> {
    fn new(object_access: T) -> Self {
        Self(object_access)
    }

    fn transactions(&self) -> RefMut<MapIndex<T, Hash, Transaction>> {
        self.0.get_object("transactions")
    }

    fn blocks(&self) -> RefMut<ListIndex<T, Hash>> {
        self.0.get_object("blocks")
    }

    fn wallets(&self) -> RefMut<ProofMapIndex<T, PublicKey, Wallet>> {
        self.0.get_object("wallets")
    }

    fn wallets_history(&self, owner: &PublicKey) -> RefMut<ProofListIndex<T, Hash>> {
        self.0.get_object(("wallets.history", owner))
    }
}

impl<T: ObjectAccess> Schema<T> {
    fn add_transaction_to_history(&self, owner: &PublicKey, tx_hash: Hash) -> Hash {
        let mut history = self.wallets_history(owner);
        history.push(tx_hash);
        history.object_hash()
    }
}

impl Block {
    fn execute(&self, db: &TemporaryDB) {
        let fork = db.fork();
        for transaction in &self.transactions {
            transaction.execute(&fork);
        }
        Schema::new(&fork).blocks().push(self.object_hash());
        db.merge(fork.into_patch()).unwrap();
    }
}

fn create_user(name: &str) -> PublicKey {
    let name = name.to_string().object_hash();
    PublicKey::from_bytes(name.as_ref().into()).unwrap()
}

fn main() {
    // Creates a database instance in the /tmp dir. It will be
    // removed when the DB object gets out of scope.
    let db = TemporaryDB::new();

    // Creates an empty genesis block.
    let genesis = Block {
        prev_block: Hash::zero(),
        transactions: Vec::new(),
    };
    genesis.execute(&db);

    // Creates user keys based on user names.
    let alice = create_user("Alice");
    let bob = create_user("Bob");

    // Creates a transaction that transfers money from Alice to Bob.
    let transaction = Transaction {
        sender: alice,
        receiver: bob,
        amount: 100,
    };
    let block = Block {
        prev_block: genesis.object_hash(),
        transactions: vec![transaction],
    };

    // Executes a block to persist our state of the blockchain in MerkleDB.
    block.execute(&db);

    // Gets a snapshot of the current database state.
    let snapshot = db.snapshot();
    let schema = Schema::new(&snapshot);

    // Checks that our users have the specified amount of money.
    let wallets = schema.wallets();
    let alice_wallet = wallets.get(&alice).unwrap();
    let bob_wallet = wallets.get(&bob).unwrap();

    assert_eq!(alice_wallet.outgoing, 100);
    assert_eq!(bob_wallet.incoming, 100);

    // Gets and checks a proof of existence of Alice's wallet in the blockchain.
    let proof = wallets.get_proof(alice);
    let checked_proof = proof.check().unwrap();
    assert_eq!(
        checked_proof.entries().collect::<Vec<_>>(),
        vec![(&alice, &alice_wallet)]
    );
}
