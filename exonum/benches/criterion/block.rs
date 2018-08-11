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

//! Benchmarking performance of creating patches for blocks (`Blockchain::create_patch` method).
//!
//! What the benchmark measures:
//!
//! - Work with the storage (reading and writing from indexes).
//! - Cryptographic operations (`ProofMapIndex` in the `cryptocurrency` bench).
//!
//! What the benchmark doesn't measure:
//!
//! - Signature verification (transactions are written directly to the node's pool, bypassing
//!   signature checks and other verification).

use criterion::{Criterion, ParameterizedBenchmark};
use exonum::{
    blockchain::{Blockchain, Schema, Service, Transaction}, crypto::{Hash, PublicKey, SecretKey},
    helpers::{Height, ValidatorId}, node::ApiSender,
    storage::{Database, DbOptions, Patch, RocksDB},
};
use futures::sync::mpsc;
use rand::{Rng, SeedableRng, XorShiftRng};
use tempdir::TempDir;

/// Number of transactions added to the blockchain before the bench begins.
const PREPARE_TRANSACTIONS: usize = 10_000;
/// Tested values for the number of transactions in an added block.
///
/// `PREPARE_TRANSACTIONS` should be divisible by all values.
const TXS_IN_BLOCK: &[usize] = &[10, 25, 50, 100];

/// Shorthand type for boxed transactions. Using `Box<Transaction>` within an `impl Iterator`
/// requires specifying the lifetime explicitly, so we do this one time here.
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

mod timestamping {
    use super::{gen_keypair_from_rng, BoxedTx};
    use exonum::{
        blockchain::{ExecutionResult, Service, Transaction}, crypto::{CryptoHash, Hash, PublicKey},
        encoding::Error as EncodingError, messages::{Message, RawTransaction},
        storage::{Fork, Snapshot},
    };
    use rand::Rng;

    const TIMESTAMPING_SERVICE_ID: u16 = 1;

    pub struct Timestamping;

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
            use exonum::blockchain::TransactionSet;
            Ok(TimestampingTransactions::tx_from_raw(raw)?.into())
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

    pub fn transactions(mut rng: impl Rng) -> impl Iterator<Item = BoxedTx> {
        (0_u32..).map(move |i| {
            let (pub_key, sec_key) = gen_keypair_from_rng(&mut rng);
            Tx::new(&pub_key, &i.hash(), &sec_key).into()
        })
    }
}

mod cryptocurrency {
    use super::{gen_keypair_from_rng, BoxedTx};
    use exonum::{
        blockchain::{ExecutionResult, Service, Transaction}, crypto::{Hash, PublicKey},
        encoding::Error as EncodingError, messages::{Message, RawTransaction},
        storage::{Fork, MapIndex, ProofMapIndex, Snapshot},
    };
    use rand::{seq::sample_slice_ref, Rng};

    const CRYPTOCURRENCY_SERVICE_ID: u16 = 255;

    // Number of generated accounts.
    const KEY_COUNT: usize = super::PREPARE_TRANSACTIONS / 10;
    // Initial balance of each account.
    const INITIAL_BALANCE: u64 = 100;

    pub struct Cryptocurrency;

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
            use exonum::blockchain::TransactionSet;
            Ok(CryptocurrencyTransactions::tx_from_raw(raw)?.into())
        }
    }

    transactions! {
        CryptocurrencyTransactions {
            const SERVICE_ID = CRYPTOCURRENCY_SERVICE_ID;

            /// Transfers one unit of currency from `from` to `to`.
            struct Tx {
                from: &PublicKey,
                to: &PublicKey,
                seed: u32,
            }

            /// Same as `Tx`, but without cryptographic proofs in `execute`.
            struct SimpleTx {
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
            let mut index = ProofMapIndex::new("provable_balances", fork);

            let from_balance = index.get(self.from()).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(self.to()).unwrap_or(INITIAL_BALANCE);
            index.put(self.from(), from_balance - 1);
            index.put(self.to(), to_balance + 1);

            Ok(())
        }
    }

    impl Transaction for SimpleTx {
        fn verify(&self) -> bool {
            self.verify_signature(self.from())
        }

        fn execute(&self, fork: &mut Fork) -> ExecutionResult {
            let mut index = MapIndex::new("balances", fork);

            let from_balance = index.get(self.from()).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(self.to()).unwrap_or(INITIAL_BALANCE);
            index.put(self.from(), from_balance - 1);
            index.put(self.to(), to_balance + 1);

            Ok(())
        }
    }

    pub fn provable_transactions(mut rng: impl Rng) -> impl Iterator<Item = BoxedTx> {
        let keys: Vec<_> = (0..KEY_COUNT)
            .map(|_| gen_keypair_from_rng(&mut rng))
            .collect();

        (0..).map(move |i| match *sample_slice_ref(&mut rng, &keys, 2) {
            [(ref from, ref from_sk), (ref to, ..)] => Tx::new(from, to, i, from_sk).into(),
            _ => unreachable!(),
        })
    }

    pub fn unprovable_transactions(mut rng: impl Rng) -> impl Iterator<Item = BoxedTx> {
        let keys: Vec<_> = (0..KEY_COUNT)
            .map(|_| gen_keypair_from_rng(&mut rng))
            .collect();

        (0..).map(move |i| match *sample_slice_ref(&mut rng, &keys, 2) {
            [(ref from, ref from_sk), (ref to, ..)] => SimpleTx::new(from, to, i, from_sk).into(),
            _ => unreachable!(),
        })
    }
}

/// Writes transactions to the pool and returns their hashes.
fn prepare_txs(blockchain: &mut Blockchain, transactions: &[BoxedTx]) -> Vec<Hash> {
    let mut fork = blockchain.fork();

    let tx_hashes = {
        let mut schema = Schema::new(&mut fork);

        // Remove all currently present transactions from the pool,
        // so that they won't clutter it eventually.
        schema.clear_transaction_pool();

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
    let transactions: Vec<_> = generator.take(txs_in_block * blockchain_height).collect();

    for i in 0..blockchain_height {
        let start = txs_in_block * i;
        let end = txs_in_block * (i + 1);
        let tx_hashes = prepare_txs(blockchain, &transactions[start..end]);
        assert_transactions_in_pool(blockchain, &tx_hashes);

        let patch = execute_block(blockchain, i as u64, &tx_hashes);
        blockchain.merge(patch).unwrap();
    }
}

fn execute_block_rocksdb(
    criterion: &mut Criterion,
    bench_name: &'static str,
    service: Box<dyn Service>,
    mut tx_generator: impl Iterator<Item = BoxedTx>,
) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    let mut blockchain = create_blockchain(db, vec![service]);

    // We don't particularly care how transactions are distributed in the blockchain
    // in the preparation phase.
    prepare_blockchain(
        &mut blockchain,
        tx_generator.by_ref(),
        PREPARE_TRANSACTIONS / TXS_IN_BLOCK[2],
        TXS_IN_BLOCK[2],
    );

    // Pre-cache transactions for the created block.
    let txs: Vec<_> = tx_generator
        .take(TXS_IN_BLOCK[TXS_IN_BLOCK.len() - 1])
        .collect();

    // Because execute_block is not really "micro benchmark"
    // executing it as regular benches, with 100 samples,
    // lead to relatively big testing time.
    // That's why the number of samples was decreased in each test.
    criterion.bench(
        bench_name,
        ParameterizedBenchmark::new(
            bench_name,
            move |bencher, &&txs_in_block| {
                let tx_hashes = prepare_txs(&mut blockchain, &txs[..txs_in_block]);
                assert_transactions_in_pool(&blockchain, &tx_hashes);

                let height: u64 = blockchain.last_block().height().next().into();
                bencher.iter(|| {
                    execute_block(&blockchain, height, &tx_hashes);
                });
            },
            TXS_IN_BLOCK,
        ).sample_size(50),
    );
}

pub fn bench_block(criterion: &mut Criterion) {
    execute_block_rocksdb(
        criterion,
        "block_timestamping",
        timestamping::Timestamping.into(),
        timestamping::transactions(XorShiftRng::from_seed([2; 16])),
    );

    execute_block_rocksdb(
        criterion,
        "block_cryptocurrency",
        cryptocurrency::Cryptocurrency.into(),
        cryptocurrency::provable_transactions(XorShiftRng::from_seed([3; 16])),
    );

    execute_block_rocksdb(
        criterion,
        "block_cryptocurrency_no_proofs",
        cryptocurrency::Cryptocurrency.into(),
        cryptocurrency::unprovable_transactions(XorShiftRng::from_seed([4; 16])),
    );
}
