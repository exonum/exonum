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

use criterion::{Benchmark, Criterion};
use exonum::{
    blockchain::{Blockchain, ExecutionResult, Schema, Service, Transaction},
    crypto::{gen_keypair, CryptoHash, Hash, PublicKey, SecretKey},
    encoding::Error as EncodingError, helpers::{Height, ValidatorId},
    messages::{Message, RawTransaction}, node::ApiSender,
    storage::{Database, DbOptions, Fork, Patch, ProofMapIndex, RocksDB, Snapshot},
};
use futures::sync::mpsc;
use tempdir::TempDir;

const TIMESTAMPING_SERVICE_ID: u16 = 1;
const CRYPTOCURRENCY_SERVICE_ID: u16 = 255;

fn create_blockchain(db: Box<Database>, services: Vec<Box<Service>>) -> Blockchain {
    let dummy_channel = mpsc::channel(1);
    let dummy_keypair = (PublicKey::zero(), SecretKey::zero());
    Blockchain::new(
        db,
        services,
        dummy_keypair.0,
        dummy_keypair.1,
        ApiSender::new(dummy_channel.0),
    )
}

fn execute_block(blockchain: &Blockchain, height: u64, txs: &[Hash]) -> Patch {
    blockchain
        .create_patch(ValidatorId::zero(), Height(height), txs)
        .1
}

fn execute_timestamping(db: Box<Database>, c: &mut Criterion) {
    struct Timestamping;

    impl Service for Timestamping {
        fn service_id(&self) -> u16 {
            TIMESTAMPING_SERVICE_ID
        }

        fn service_name(&self) -> &'static str {
            "timestamping"
        }

        fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
            Vec::new()
        }

        fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
            Ok(Box::new(Tx::from_raw(raw)?))
        }
    }

    transactions! {
        TimestampingTransactions {
            const SERVICE_ID = TIMESTAMPING_SERVICE_ID;
            struct Tx {
                from: &PublicKey,
                data: &Hash,
            }
        }
    }

    impl Transaction for Tx {
        fn verify(&self) -> bool {
            self.verify_signature(self.from())
        }

        fn execute(&self, _: &mut Fork) -> ExecutionResult {
            Ok(())
        }
    }

    fn prepare_txs(blockchain: &mut Blockchain, height: u64, count: u64) -> Vec<Hash> {
        let mut fork = blockchain.fork();
        let mut txs = Vec::new();
        {
            let mut schema = Schema::new(&mut fork);
            let (pub_key, sec_key) = gen_keypair();
            for i in (height * count)..((height + 1) * count) {
                let tx = Tx::new(&pub_key, &i.hash(), &sec_key);
                let tx_hash = Transaction::hash(&tx);
                txs.push(tx_hash);
                schema.add_transaction_into_pool(tx.raw().clone());
            }
        }
        blockchain.merge(fork.into_patch()).unwrap();
        txs
    }
    let mut blockchain = create_blockchain(db, vec![Box::new(Timestamping)]);
    for i in 0..100 {
        let txs = prepare_txs(&mut blockchain, i, 1000);
        let patch = execute_block(&blockchain, i, &txs);
        blockchain.merge(patch).unwrap();
    }

    let txs = prepare_txs(&mut blockchain, 100, 1000);

    c.bench(
        "timestamping",
        Benchmark::new("timestamping", move |b| {
            b.iter(|| execute_block(&blockchain, 100, &txs))
        }).sample_size(16),
    );
}

fn execute_cryptocurrency(db: Box<Database>, c: &mut Criterion) {
    const TRANSACTIONS_IN_BLOCK: u64 = 100;
    // Number of generated accounts.
    const KEY_COUNT: usize = TRANSACTIONS_IN_BLOCK as usize * 10;
    // Initial balance of each account.
    const INITIAL_BALANCE: u64 = 100;
    // The final blockchain height.
    const HEIGHT: u64 = 100;

    struct Cryptocurrency;

    impl Service for Cryptocurrency {
        fn service_id(&self) -> u16 {
            255
        }

        fn service_name(&self) -> &'static str {
            "cryptocurrency"
        }

        fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
            Vec::new()
        }

        fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
            Ok(Box::new(Tx::from_raw(raw)?))
        }
    }

    transactions! {
        CryptocurrencyTransactions {
            const SERVICE_ID = CRYPTOCURRENCY_SERVICE_ID;
            struct Tx {
                from: &PublicKey,
                to: &PublicKey,
            }
        }
    }

    impl Transaction for Tx {
        fn verify(&self) -> bool {
            self.verify_signature(self.from())
        }

        fn execute(&self, fork: &mut Fork) -> ExecutionResult {
            let mut index = ProofMapIndex::new("balances_txs", fork);

            let from_balance = index.get(self.from()).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(self.to()).unwrap_or(INITIAL_BALANCE);
            index.put(self.from(), from_balance - 1);
            index.put(self.to(), to_balance + 1);

            Ok(())
        }
    }

    fn prepare_txs(
        blockchain: &mut Blockchain,
        height: u64,
        count: u64,
        keys: &[(PublicKey, SecretKey)],
    ) -> Vec<Hash> {
        let mut fork = blockchain.fork();
        let txs: Vec<_> = {
            let mut schema = Schema::new(&mut fork);

            (0..count)
                .map(|x| x + height * count)
                .map(|i| {
                    // gcd(7, KEY_COUNT) == 1. This ensures that the recipient for each
                    // height is different, even if the sender is the same, which is
                    // required for proper emulation of transaction processing - we shouldn't
                    // process the same transaction multiple times, as this never occurs
                    // in the "real" blockchain.
                    let shift = KEY_COUNT / 7 * (height as usize + 1);
                    let tx = Tx::new(
                        &keys[i as usize % KEY_COUNT].0,
                        &keys[(i as usize + shift) % KEY_COUNT].0,
                        &keys[i as usize % KEY_COUNT].1,
                    );

                    schema.add_transaction_into_pool(tx.raw().clone());
                    Transaction::hash(&tx)
                })
                .collect()
        };
        blockchain.merge(fork.into_patch()).unwrap();
        txs
    }

    // Ensure proper transaction processing. These assertions are performed before
    // the benchmark and do not influence its timings.
    fn assert_transactions_in_pool(blockchain: &Blockchain, txs: &[Hash]) {
        let snapshot = blockchain.snapshot();
        let schema = Schema::new(&snapshot);

        assert!(
            txs.iter()
                .all(|hash| schema.transactions_pool().contains(&hash)
                    && !schema.transactions_locations().contains(&hash))
        );
    }

    let mut blockchain = create_blockchain(db, vec![Box::new(Cryptocurrency)]);
    let keys: Vec<_> = (0..KEY_COUNT).map(|_| gen_keypair()).collect();

    for i in 0..HEIGHT {
        let txs = prepare_txs(&mut blockchain, i, TRANSACTIONS_IN_BLOCK, &keys);
        assert_transactions_in_pool(&blockchain, &txs);

        let patch = execute_block(&blockchain, i, &txs);
        blockchain.merge(patch).unwrap();
    }

    let txs = prepare_txs(&mut blockchain, HEIGHT, TRANSACTIONS_IN_BLOCK, &keys);
    assert_transactions_in_pool(&blockchain, &txs);
    c.bench(
        "cryptocurrency",
        Benchmark::new("cryptocurrency", move |b| {
            b.iter(|| execute_block(&blockchain, HEIGHT, &txs))
        }).sample_size(50),
    );
}

fn create_rocksdb(tempdir: &TempDir) -> Box<Database> {
    let options = DbOptions::default();
    let db = Box::new(RocksDB::open(tempdir.path(), &options).unwrap());
    db as Box<Database>
}

fn bench_execute_block_timestamping_rocksdb(c: &mut Criterion) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    execute_timestamping(db, c)
}

fn bench_execute_block_cryptocurrency_rocksdb(c: &mut Criterion) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    execute_cryptocurrency(db, c)
}

pub fn bench_block(c: &mut Criterion) {
    // Because execute_block is not really "micro benchmark"
    // executing it as regular benches, with 100 samples,
    // lead to relatively big testing time.
    // That's why, number of samples was decreased in each test.

    bench_execute_block_timestamping_rocksdb(c);
    bench_execute_block_cryptocurrency_rocksdb(c);
}
