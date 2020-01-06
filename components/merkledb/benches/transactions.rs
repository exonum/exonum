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

use criterion::{AxisScale, Bencher, Criterion, PlotConfiguration, Throughput};
use exonum_crypto::{Hash, PublicKey, PUBLIC_KEY_LENGTH};
use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
use rand::{rngs::StdRng, Rng, RngCore, SeedableRng};
use serde_derive::{Deserialize, Serialize};

use std::{collections::HashMap, fmt};

use exonum_merkledb::{
    access::{Access, FromAccess},
    BinaryValue, Database, Fork, Group, ListIndex, MapIndex, ObjectHash, ProofListIndex,
    ProofMapIndex, TemporaryDB,
};

const SEED: [u8; 32] = [100; 32];
const SAMPLE_SIZE: usize = 10;

#[cfg(not(feature = "long_benchmarks"))]
const ITEM_COUNTS: &[BenchParams] = &[
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
#[cfg(not(feature = "long_benchmarks"))]
const TOTAL_TX_COUNT: u64 = 10_000;

#[cfg(feature = "long_benchmarks")]
const ITEM_COUNTS: &[BenchParams] = &[
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
];
#[cfg(feature = "long_benchmarks")]
const TOTAL_TX_COUNT: u64 = 100_000;

#[derive(Clone, Copy, Debug)]
struct BenchParams {
    users: usize,
    blocks: usize,
    txs_in_block: usize,
}

impl fmt::Display for BenchParams {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "users = {}, blocks = {} x {} txs",
            self.users, self.blocks, self.txs_in_block
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
struct Wallet {
    incoming: u32,
    outgoing: u32,
    history_root: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
struct Transaction {
    sender: PublicKey,
    receiver: PublicKey,
    amount: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
struct Block {
    transactions: Vec<Transaction>,
}

impl Transaction {
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

#[derive(FromAccess)]
struct Schema<T: Access> {
    transactions: MapIndex<T::Base, Hash, Transaction>,
    blocks: ListIndex<T::Base, Hash>,
    wallets: ProofMapIndex<T::Base, PublicKey, Wallet>,
    wallet_history: Group<T, PublicKey, ProofListIndex<T::Base, Hash>>,
}

impl<T: Access> Schema<T> {
    fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

impl Schema<&Fork> {
    fn add_transaction_to_history(&self, owner: &PublicKey, tx_hash: Hash) -> Hash {
        let mut history = self.wallet_history.get(owner);
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
        Schema::new(&fork).blocks.push(self.object_hash());
        db.merge(fork.into_patch()).unwrap();
    }

    fn execute_with_isolation(&self, db: &TemporaryDB) {
        let mut rng = StdRng::from_seed(SEED);

        let mut fork = db.fork();
        for transaction in &self.transactions {
            transaction.execute(&fork);
            if rng.gen::<u8>() % 16 == 0 {
                fork.rollback();
            } else {
                fork.flush();
            }
        }
        Schema::new(&fork).blocks.push(self.object_hash());
        db.merge(fork.into_patch()).unwrap();
    }
}

fn gen_random_blocks(blocks: usize, txs_count: usize, wallets_count: usize) -> Vec<Block> {
    let mut rng = StdRng::from_seed(SEED);
    let users = (0..wallets_count)
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

fn do_bench(bencher: &mut Bencher, params: BenchParams, isolate: bool) {
    let blocks = gen_random_blocks(params.blocks, params.txs_in_block, params.users);
    bencher.iter_with_setup(TemporaryDB::new, |db| {
        for block in &blocks {
            if isolate {
                block.execute_with_isolation(&db);
            } else {
                block.execute(&db);
            }
        }

        // Some fast assertions.
        let snapshot = db.snapshot();
        let schema = Schema::new(&snapshot);
        assert_eq!(schema.blocks.len(), params.blocks as u64);
    });
}

pub fn bench_transactions(c: &mut Criterion) {
    exonum_crypto::init();

    let mut group = c.benchmark_group("plain_transactions");
    group
        .throughput(Throughput::Elements(TOTAL_TX_COUNT))
        .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
        .sample_size(SAMPLE_SIZE);

    for &params in ITEM_COUNTS {
        group.bench_function(params.to_string(), |bencher| {
            do_bench(bencher, params, false);
        });
    }
    group.finish();

    let mut group = c.benchmark_group("isolated_transactions");
    group
        .throughput(Throughput::Elements(TOTAL_TX_COUNT))
        .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
        .sample_size(SAMPLE_SIZE);

    for &params in ITEM_COUNTS {
        group.bench_function(params.to_string(), |bencher| {
            do_bench(bencher, params, true);
        });
    }
    group.finish();
}
