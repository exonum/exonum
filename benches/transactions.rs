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
use rand::{Rng, RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};

use exonum_crypto::{Hash, PublicKey, PUBLIC_KEY_LENGTH};
use exonum_merkledb::{
    BinaryValue, Database, Fork, IndexAccess, ListIndex, MapIndex, ProofListIndex, ProofMapIndex,
    TemporaryDB, UniqueHash,
};

const SEED: [u8; 16] = [100; 16];
const USERS_COUNT: usize = 10_000;
const SAMPLE_SIZE: usize = 10;

#[cfg(all(test, not(feature = "long_benchmarks")))]
const ITEM_COUNT: [BenchParams; 4] = [
    BenchParams {
        blocks_count: 1,
        txs_in_block_count: 10_000,
    },
    BenchParams {
        blocks_count: 10,
        txs_in_block_count: 1_000,
    },
    BenchParams {
        blocks_count: 100,
        txs_in_block_count: 100,
    },
    BenchParams {
        blocks_count: 1_000,
        txs_in_block_count: 10,
    },
];

#[cfg(all(test, feature = "long_benchmarks"))]
const ITEM_COUNT: [BenchParams; 5] = [
    BenchParams {
        blocks_count: 1,
        txs_in_block_count: 10_000,
    },
    BenchParams {
        blocks_count: 10,
        txs_in_block_count: 1_000,
    },
    BenchParams {
        blocks_count: 100,
        txs_in_block_count: 100,
    },
    BenchParams {
        blocks_count: 1_000,
        txs_in_block_count: 10,
    },
    BenchParams {
        blocks_count: 1_000,
        txs_in_block_count: 1_000,
    },
];

#[derive(Clone, Copy, Debug)]
struct BenchParams {
    blocks_count: usize,
    txs_in_block_count: usize,
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

impl UniqueHash for Wallet {}

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

impl UniqueHash for Transaction {}

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

impl UniqueHash for Block {}

struct Schema<T: IndexAccess>(T);

impl<T: IndexAccess> Schema<T> {
    fn new(index_access: T) -> Self {
        Self(index_access)
    }

    fn transactions(&self) -> MapIndex<T, Hash, Transaction> {
        MapIndex::new("transactions", self.0.clone())
    }

    fn blocks(&self) -> ListIndex<T, Hash> {
        ListIndex::new("blocks", self.0.clone())
    }

    fn wallets(&self) -> ProofMapIndex<T, PublicKey, Wallet> {
        ProofMapIndex::new("wallets", self.0.clone())
    }

    fn wallets_history(&self, owner: &PublicKey) -> ProofListIndex<T, Hash> {
        ProofListIndex::new_in_family("wallets.history", owner, self.0.clone())
    }
}

impl Schema<&Fork> {
    fn add_transaction_to_history(&self, owner: &PublicKey, tx_hash: Hash) -> Hash {
        let mut history = self.wallets_history(owner);
        history.push(tx_hash);
        history.list_hash()
    }
}

impl Transaction {
    fn execute(&self, fork: &Fork) {
        let tx_hash = self.hash();

        let schema = Schema::new(fork);
        schema.transactions().put(&self.hash(), *self);

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

impl Block {
    fn execute(&self, db: &TemporaryDB) {
        let fork = db.fork();
        for transaction in &self.transactions {
            transaction.execute(&fork);
        }
        Schema::new(&fork).blocks().push(self.hash());
        db.merge(fork.into_patch()).unwrap();
    }
}

fn gen_random_blocks(blocks_count: usize, txs_count: usize, wallets_count: usize) -> Vec<Block> {
    let mut rng = XorShiftRng::from_seed(SEED);
    let users = (0..wallets_count)
        .into_iter()
        .map(|idx| {
            let mut base = [0; PUBLIC_KEY_LENGTH];
            rng.fill_bytes(&mut base);
            (idx, PublicKey::from_bytes(base.as_ref().into()).unwrap())
        })
        .collect::<HashMap<_, _>>();

    let get_random_user = |rng: &mut XorShiftRng| -> PublicKey {
        let id = rng.gen_range(0, wallets_count);
        *users.get(&id).unwrap()
    };

    (0..blocks_count)
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
                let db = TemporaryDB::new();
                let blocks =
                    gen_random_blocks(params.blocks_count, params.txs_in_block_count, USERS_COUNT);
                b.iter(|| {
                    for block in &blocks {
                        block.execute(&db)
                    }
                })
            },
            item_counts,
        )
        .throughput(|&s| Throughput::Elements((s.txs_in_block_count * s.blocks_count) as u32))
        .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
        .sample_size(SAMPLE_SIZE),
    );
}
