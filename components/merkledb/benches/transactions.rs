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

use std::{borrow::Cow, collections::HashMap};

use criterion::{
    AxisScale, Bencher, Criterion, ParameterizedBenchmark, PlotConfiguration, Throughput,
};
use rand::{rngs::StdRng, Rng, RngCore, SeedableRng};
use serde_derive::{Deserialize, Serialize};

use exonum_crypto::{Hash, PublicKey, PUBLIC_KEY_LENGTH};
use exonum_merkledb::{
    impl_object_hash_for_binary_value, BinaryValue, Database, Fork, ListIndex, MapIndex,
    ObjectAccess, ObjectHash, ProofListIndex, ProofMapIndex, RefMut, TemporaryDB,
};

const SEED: [u8; 32] = [100; 32];
const SAMPLE_SIZE: usize = 10;

#[cfg(all(test, not(feature = "long_benchmarks")))]
const ITEM_COUNT: [BenchParams; 10] = [
    BenchParams {
        users: 10_000,
        blocks: 1,
        txs_in_block: 10_000,
    },
    BenchParams {
        users: 100,
        blocks: 1,
        txs_in_block: 10_000,
    },
    BenchParams {
        users: 10_000,
        blocks: 10,
        txs_in_block: 1_000,
    },
    BenchParams {
        users: 100,
        blocks: 10,
        txs_in_block: 1_000,
    },
    BenchParams {
        users: 10_000,
        blocks: 100,
        txs_in_block: 100,
    },
    BenchParams {
        users: 100,
        blocks: 100,
        txs_in_block: 100,
    },
    BenchParams {
        users: 10_000,
        blocks: 1_000,
        txs_in_block: 10,
    },
    BenchParams {
        users: 100,
        blocks: 1_000,
        txs_in_block: 10,
    },
    BenchParams {
        users: 10_000,
        blocks: 10_000,
        txs_in_block: 1,
    },
    BenchParams {
        users: 100,
        blocks: 10_000,
        txs_in_block: 1,
    },
];

#[cfg(all(test, feature = "long_benchmarks"))]
const ITEM_COUNT: [BenchParams; 6] = [
    BenchParams {
        users: 1_000,
        blocks: 10,
        txs_in_block: 10_000,
    },
    BenchParams {
        users: 1_000,
        blocks: 100,
        txs_in_block: 1_000,
    },
    BenchParams {
        users: 1_000,
        blocks: 1_000,
        txs_in_block: 100,
    },
    BenchParams {
        users: 1_000,
        blocks: 10_000,
        txs_in_block: 10,
    },
    BenchParams {
        users: 1_000,
        blocks: 100_000,
        txs_in_block: 1,
    },
    BenchParams {
        users: 1_000,
        blocks: 1_000,
        txs_in_block: 1_000,
    },
];

#[derive(Clone, Copy, Debug)]
struct BenchParams {
    users: usize,
    blocks: usize,
    txs_in_block: usize,
}

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

        let schema = RefSchema::new(fork);
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

struct RefSchema<T: ObjectAccess>(T);

impl<T: ObjectAccess> RefSchema<T> {
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

impl<T: ObjectAccess> RefSchema<T> {
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
        RefSchema::new(&fork).blocks().push(self.object_hash());
        db.merge(fork.into_patch()).unwrap();
    }
}

fn gen_random_blocks(blocks: usize, txs_count: usize, wallets_count: usize) -> Vec<Block> {
    let mut rng: StdRng = SeedableRng::from_seed(SEED);
    let users = (0..wallets_count)
        .into_iter()
        .map(|idx| {
            let mut base = [0; PUBLIC_KEY_LENGTH];
            rng.fill_bytes(&mut base);
            (idx, PublicKey::from_bytes(base.as_ref().into()).unwrap())
        })
        .collect::<HashMap<_, _>>();

    let get_random_user = |rng: &mut StdRng| -> PublicKey {
        let id = rng.gen_range(0, wallets_count);
        *users.get(&id).unwrap()
    };

    (0..blocks)
        .into_iter()
        .map(move |_| {
            let transactions = (0..txs_count)
                .map(|_| Transaction {
                    sender: get_random_user(&mut rng),
                    receiver: get_random_user(&mut rng),
                    amount: rng.gen_range(0, 10),
                })
                .collect();
            Block { transactions }
        })
        .collect()
}

pub fn bench_transactions(c: &mut Criterion) {
    exonum_crypto::init();

    let item_counts = ITEM_COUNT.iter().cloned();
    c.bench(
        "transactions",
        ParameterizedBenchmark::new(
            "currency_like",
            move |b: &mut Bencher, params: &BenchParams| {
                let blocks = gen_random_blocks(params.blocks, params.txs_in_block, params.users);
                b.iter_with_setup(TemporaryDB::new, |db| {
                    for block in &blocks {
                        block.execute(&db)
                    }
                    // Some fast assertions.
                    let snapshot = db.snapshot();
                    let schema = RefSchema::new(&snapshot);
                    assert_eq!(schema.blocks().len(), params.blocks as u64);
                })
            },
            item_counts,
        )
        .throughput(|&s| Throughput::Elements((s.txs_in_block * s.blocks) as u32))
        .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
        .sample_size(SAMPLE_SIZE),
    );
}
