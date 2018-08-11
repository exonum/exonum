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

use criterion::{ParameterizedBenchmark, Criterion};
use exonum::{
    blockchain::{Blockchain, ExecutionResult, Schema, Service, Transaction},
    crypto::{CryptoHash, Hash, PublicKey, SecretKey}, encoding::Error as EncodingError,
    helpers::{Height, ValidatorId}, messages::{Message, RawTransaction}, node::ApiSender,
    storage::{Database, DbOptions, Fork, Patch, ProofMapIndex, RocksDB, Snapshot},
};
use futures::sync::mpsc;
use rand::{seq::sample_slice_ref, Rng, SeedableRng, XorShiftRng};
use tempdir::TempDir;

const TIMESTAMPING_SERVICE_ID: u16 = 1;
const CRYPTOCURRENCY_SERVICE_ID: u16 = 255;

const TOTAL_TRANSACTIONS: usize = 10_000;
const TXS_IN_BLOCK: &[usize] = &[10, 25, 50, 100, 200];

// Shorthand type for boxed transactions. Using `Box<Transaction>` within an `impl Iterator`
// requires specifying the lifetime explicitly, so we do this one time here.
type BoxedTx = Box<dyn Transaction + 'static>;

fn gen_keypair_from_rng<R: Rng>(rng: &mut R) -> (PublicKey, SecretKey) {
    use exonum::crypto::{gen_keypair_from_seed, Seed, SEED_LENGTH};

    let mut bytes = [0_u8; SEED_LENGTH];
    rng.fill(&mut bytes);
    gen_keypair_from_seed(&Seed::new(bytes))
}

fn create_rocksdb(tempdir: &TempDir) -> RocksDB {
    let options = DbOptions::default();
    RocksDB::open(tempdir.path(), &options).unwrap()
}

fn create_blockchain(db: impl Database, services: Vec<Box<Service>>) -> Blockchain {
    use std::sync::Arc;

    let dummy_channel = mpsc::channel(1);
    let dummy_keypair = (PublicKey::zero(), SecretKey::zero());
    Blockchain::new(
        Arc::new(db) as Arc<dyn Database>,
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

fn timestamping() -> (Box<dyn Service>, impl Iterator<Item = BoxedTx>) {
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

        fn tx_from_raw(&self, raw: RawTransaction) -> Result<BoxedTx, EncodingError> {
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

    let mut rng = XorShiftRng::from_seed([2_u8; 16]);

    let generator = (0..).map(move |i| {
        let (pub_key, sec_key) = gen_keypair_from_rng(&mut rng);
        Tx::new(&pub_key, &i.hash(), &sec_key).into()
    });
    (Box::new(Timestamping), generator)
}

fn cryptocurrency() -> (Box<dyn Service>, impl Iterator<Item = BoxedTx>) {
    // Number of generated accounts.
    const KEY_COUNT: usize = TOTAL_TRANSACTIONS / 10;
    // Initial balance of each account.
    const INITIAL_BALANCE: u64 = 100;

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

        fn tx_from_raw(&self, raw: RawTransaction) -> Result<BoxedTx, EncodingError> {
            Ok(Box::new(Tx::from_raw(raw)?))
        }
    }

    transactions! {
        CryptocurrencyTransactions {
            const SERVICE_ID = CRYPTOCURRENCY_SERVICE_ID;
            struct Tx {
                from: &PublicKey,
                to: &PublicKey,
                seed: u32,
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

    let mut rng = XorShiftRng::from_seed([1; 16]);
    let keys: Vec<_> = (0..KEY_COUNT)
        .map(|_| gen_keypair_from_rng(&mut rng))
        .collect();

    let tx_generator = (0..).map(move |i| match *sample_slice_ref(&mut rng, &keys, 2) {
        [(ref from, ref from_sk), (ref to, ..)] => Tx::new(from, to, i, from_sk).into(),
        _ => unreachable!(),
    });

    (Box::new(Cryptocurrency), tx_generator)
}

/// Writes transactions to the pool and returns their hashes.
fn prepare_txs(blockchain: &mut Blockchain, transactions: &[BoxedTx]) -> Vec<Hash> {
    let mut fork = blockchain.fork();

    let tx_hashes = {
        let mut schema = Schema::new(&mut fork);

        transactions
            .iter()
            .map(|tx| {
                schema.add_transaction_into_pool(tx.raw().clone());
                tx.hash()
            })
            .collect()
    };

    blockchain.merge(fork.into_patch()).unwrap();
    tx_hashes
}

/// Verifies that transactions with the specified hashes are present in the pool.
///
/// We do this to ensure proper transaction processing. The assertions are performed before
/// the benchmark and do not influence its timings.
fn assert_transactions_in_pool(blockchain: &Blockchain, tx_hashes: &[Hash]) {
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);

    assert!(
        tx_hashes
            .iter()
            .all(|hash| schema.transactions_pool().contains(&hash)
                && !schema.transactions_locations().contains(&hash))
    );
    assert_eq!(tx_hashes.len() as u64, schema.transactions_pool_len());
}

fn prepare_blockchain(
    blockchain: &mut Blockchain,
    generator: impl Iterator<Item = BoxedTx>,
    blockchain_height: usize,
    txs_in_block: usize,
) {
    let transactions: Vec<_> = generator
        .take(txs_in_block * blockchain_height)
        .collect();

    for i in 0..blockchain_height {
        let start = txs_in_block * i;
        let end = txs_in_block * (i + 1);
        let tx_hashes = prepare_txs(blockchain, &transactions[start..end]);
        assert_transactions_in_pool(blockchain, &tx_hashes);

        let patch = execute_block(blockchain, i as u64, &tx_hashes);
        blockchain.merge(patch).unwrap();
    }
}

fn execute_block_rocksdb<F, I>(criterion: &mut Criterion, init: F, bench_name: &'static str)
where
    F: FnOnce() -> (Box<dyn Service>, I),
    I: Iterator<Item = BoxedTx> + 'static,
{
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    let (service, mut tx_generator) = init();
    let mut blockchain = create_blockchain(db, vec![service]);

    // We don't particularly care how transactions are distributed in the blockchain
    // in the preparation phase.
    prepare_blockchain(
        &mut blockchain,
        tx_generator.by_ref(),
        TOTAL_TRANSACTIONS / TXS_IN_BLOCK[2],
        TXS_IN_BLOCK[2],
    );

    let txs: Vec<_> = tx_generator.take(TXS_IN_BLOCK[TXS_IN_BLOCK.len() - 1]).collect();

    // Because execute_block is not really "micro benchmark"
    // executing it as regular benches, with 100 samples,
    // lead to relatively big testing time.
    // That's why the number of samples was decreased in each test.
    criterion.bench(
        bench_name,
        ParameterizedBenchmark::new(bench_name, move |bencher, &&txs_in_block| {
            let tx_hashes = prepare_txs(&mut blockchain, &txs[0..txs_in_block]);
            assert_transactions_in_pool(&blockchain, &tx_hashes);

            let height: u64 = blockchain.last_block().height().next().into();
            bencher.iter(|| {
                execute_block(&blockchain, height, &tx_hashes);
            });
        }, TXS_IN_BLOCK).sample_size(50),
    );
}

pub fn bench_block(criterion: &mut Criterion) {
    execute_block_rocksdb(criterion, timestamping, "block_timestamping");
    execute_block_rocksdb(criterion, cryptocurrency, "block_cryptocurrency");
}
