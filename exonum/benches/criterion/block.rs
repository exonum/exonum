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
//!
//! # Constituent benchmarks
//!
//! - `block_timestamping`: Valid no-op transactions. Measures the efficiency of operations
//!   on core indexes (e.g., recording transaction location and result).
//! - `block_timestamping_panic`: Panicking no-op transactions. Measures the efficiency of
//!   `panic` handling.
//! - `block_cryptocurrency`: Transferring cryptocurrency among random accounts. Accounts are
//!   stored in a `ProofMapIndex`.
//! - `block_cryptocurrency_no_proofs`: Transferring cryptocurrency among random accounts.
//!   Accounts are stored in a `MapIndex`.
//! - `block_cryptocurrency_rollback`: Transferring cryptocurrency among random accounts.
//!   Accounts are stored in a `MapIndex`. Transactions are rolled back 50% of the time.

use criterion::{Criterion, ParameterizedBenchmark, Throughput};
use exonum::{
    blockchain::{Blockchain, Schema, Service, Transaction},
    crypto::{Hash, PublicKey, SecretKey},
    helpers::{Height, ValidatorId},
    messages::{RawTransaction, Signed},
    node::ApiSender,
    storage::{Database, DbOptions, Patch, RocksDB},
};
use futures::sync::mpsc;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use tempdir::TempDir;

use std::iter;

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
    use exonum::{
        blockchain::{GenesisConfig, ValidatorKeys},
        crypto,
    };
    use std::sync::Arc;

    let dummy_channel = mpsc::channel(1);
    let service_keypair = (PublicKey::zero(), SecretKey::zero());
    let mut blockchain = Blockchain::new(
        Arc::new(db) as Arc<dyn Database>,
        services,
        service_keypair.0,
        service_keypair.1,
        ApiSender::new(dummy_channel.0),
    );

    let consensus_keypair = crypto::gen_keypair();
    let config = GenesisConfig::new(iter::once(ValidatorKeys {
        consensus_key: consensus_keypair.0,
        service_key: service_keypair.0,
    }));
    blockchain.initialize(config).unwrap();

    blockchain
}

fn execute_block(blockchain: &Blockchain, height: u64, txs: &[Hash]) -> (Hash, Patch) {
    blockchain.create_patch(ValidatorId::zero(), Height(height), txs)
}

mod timestamping {
    use super::{gen_keypair_from_rng, BoxedTx};
    use crate::proto;
    use exonum::{
        blockchain::{ExecutionResult, Service, Transaction, TransactionContext},
        crypto::{CryptoHash, Hash, PublicKey, SecretKey},
        messages::{Message, RawTransaction, Signed},
        storage::Snapshot,
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

        fn tx_from_raw(&self, raw: RawTransaction) -> Result<BoxedTx, failure::Error> {
            use exonum::blockchain::TransactionSet;
            Ok(TimestampingTransactions::tx_from_raw(raw)?.into())
        }
    }

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::TimestampTx")]
    struct Tx {
        data: Hash,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::TimestampTx")]
    struct PanickingTx {
        data: Hash,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
    enum TimestampingTransactions {
        Tx(Tx),
        PanickingTx(PanickingTx),
    }

    impl Tx {
        #[doc(hidden)]
        pub fn sign(pk: &PublicKey, &data: &Hash, sk: &SecretKey) -> Signed<RawTransaction> {
            Message::sign_transaction(Tx { data }, TIMESTAMPING_SERVICE_ID, *pk, sk)
        }
    }

    impl PanickingTx {
        #[doc(hidden)]
        pub fn sign(pk: &PublicKey, data: &Hash, sk: &SecretKey) -> Signed<RawTransaction> {
            Message::sign_transaction(
                PanickingTx { data: *data },
                TIMESTAMPING_SERVICE_ID,
                *pk,
                sk,
            )
        }
    }

    impl Transaction for Tx {
        fn execute(&self, _: TransactionContext) -> ExecutionResult {
            Ok(())
        }
    }

    impl Transaction for PanickingTx {
        fn execute(&self, _: TransactionContext) -> ExecutionResult {
            panic!("panic text");
        }
    }

    pub fn transactions(mut rng: impl Rng) -> impl Iterator<Item = Signed<RawTransaction>> {
        (0_u32..).map(move |i| {
            let (pub_key, sec_key) = gen_keypair_from_rng(&mut rng);
            Tx::sign(&pub_key, &i.hash(), &sec_key)
        })
    }

    pub fn panicking_transactions(
        mut rng: impl Rng,
    ) -> impl Iterator<Item = Signed<RawTransaction>> {
        (0_u32..).map(move |i| {
            let (pub_key, sec_key) = gen_keypair_from_rng(&mut rng);
            PanickingTx::sign(&pub_key, &i.hash(), &sec_key)
        })
    }
}

mod cryptocurrency {
    use super::{gen_keypair_from_rng, BoxedTx};
    use crate::proto;
    use exonum::{
        blockchain::{ExecutionError, ExecutionResult, Service, Transaction, TransactionContext},
        crypto::{Hash, PublicKey, SecretKey},
        messages::{Message, RawTransaction, Signed},
        storage::{MapIndex, ProofMapIndex, Snapshot},
    };
    use rand::{seq::SliceRandom, Rng};

    const CRYPTOCURRENCY_SERVICE_ID: u16 = 255;

    // Number of generated accounts.
    const KEY_COUNT: usize = super::PREPARE_TRANSACTIONS / 10;
    // Initial balance of each account.
    const INITIAL_BALANCE: u64 = 100;

    pub struct Cryptocurrency;

    impl Service for Cryptocurrency {
        fn service_id(&self) -> u16 {
            CRYPTOCURRENCY_SERVICE_ID
        }

        fn service_name(&self) -> &'static str {
            "cryptocurrency"
        }

        fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
            Vec::new()
        }

        fn tx_from_raw(&self, raw: RawTransaction) -> Result<BoxedTx, failure::Error> {
            use exonum::blockchain::TransactionSet;
            Ok(CryptocurrencyTransactions::tx_from_raw(raw)?.into())
        }
    }

    /// Transfers one unit of currency from `from` to `to`.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::CurrencyTx")]
    struct Tx {
        to: PublicKey,
        seed: u32,
    }

    /// Same as `Tx`, but without cryptographic proofs in `execute`.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::CurrencyTx")]
    struct SimpleTx {
        to: PublicKey,
        seed: u32,
    }

    /// Same as `SimpleTx`, but signals an error 50% of the time.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::CurrencyTx")]
    struct RollbackTx {
        to: PublicKey,
        seed: u32,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
    enum CryptocurrencyTransactions {
        Tx(Tx),
        SimpleTx(SimpleTx),
        RollbackTx(RollbackTx),
    }

    impl Tx {
        #[doc(hidden)]
        pub fn sign(
            pk: &PublicKey,
            to: &PublicKey,
            seed: u32,
            sk: &SecretKey,
        ) -> Signed<RawTransaction> {
            Message::sign_transaction(Tx { to: *to, seed }, CRYPTOCURRENCY_SERVICE_ID, *pk, sk)
        }
    }

    impl SimpleTx {
        #[doc(hidden)]
        pub fn sign(
            pk: &PublicKey,
            to: &PublicKey,
            seed: u32,
            sk: &SecretKey,
        ) -> Signed<RawTransaction> {
            Message::sign_transaction(
                SimpleTx { to: *to, seed },
                CRYPTOCURRENCY_SERVICE_ID,
                *pk,
                sk,
            )
        }
    }

    impl RollbackTx {
        #[doc(hidden)]
        pub fn sign(
            pk: &PublicKey,
            to: &PublicKey,
            seed: u32,
            sk: &SecretKey,
        ) -> Signed<RawTransaction> {
            Message::sign_transaction(
                RollbackTx { to: *to, seed },
                CRYPTOCURRENCY_SERVICE_ID,
                *pk,
                sk,
            )
        }
    }

    impl Transaction for Tx {
        fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
            let from = context.author();
            let mut index = ProofMapIndex::new("provable_balances", context.fork());

            let from_balance = index.get(&from).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(&self.to).unwrap_or(INITIAL_BALANCE);
            index.put(&from, from_balance - 1);
            index.put(&self.to, to_balance + 1);

            Ok(())
        }
    }

    impl Transaction for SimpleTx {
        fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
            let from = context.author();

            let mut index = MapIndex::new("balances", context.fork());

            let from_balance = index.get(&from).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(&self.to).unwrap_or(INITIAL_BALANCE);
            index.put(&from, from_balance - 1);
            index.put(&self.to, to_balance + 1);

            Ok(())
        }
    }

    impl Transaction for RollbackTx {
        fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
            let from = context.author();

            let mut index = MapIndex::new("balances", context.fork());

            let from_balance = index.get(&from).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(&self.to).unwrap_or(INITIAL_BALANCE);
            index.put(&from, from_balance - 1);
            index.put(&self.to, to_balance + 1);

            // We deliberately perform the check *after* reads/writes in order
            // to check efficiency of rolling the changes back.
            if self.seed % 2 == 0 {
                Ok(())
            } else {
                Err(ExecutionError::new(1))
            }
        }
    }

    pub fn provable_transactions(
        mut rng: impl Rng,
    ) -> impl Iterator<Item = Signed<RawTransaction>> {
        let keys: Vec<_> = (0..KEY_COUNT)
            .map(|_| gen_keypair_from_rng(&mut rng))
            .collect();

        (0..).map(
            move |i| match &keys.choose_multiple(&mut rng, 2).collect::<Vec<_>>()[..] {
                [(ref from, ref from_sk), (ref to, ..)] => Tx::sign(from, to, i, from_sk).into(),
                _ => unreachable!(),
            },
        )
    }

    pub fn unprovable_transactions(
        mut rng: impl Rng,
    ) -> impl Iterator<Item = Signed<RawTransaction>> {
        let keys: Vec<_> = (0..KEY_COUNT)
            .map(|_| gen_keypair_from_rng(&mut rng))
            .collect();

        (0..).map(
            move |i| match &keys.choose_multiple(&mut rng, 2).collect::<Vec<_>>()[..] {
                [(ref from, ref from_sk), (ref to, ..)] => SimpleTx::sign(from, to, i, from_sk),
                _ => unreachable!(),
            },
        )
    }

    pub fn rollback_transactions(
        mut rng: impl Rng,
    ) -> impl Iterator<Item = Signed<RawTransaction>> {
        let keys: Vec<_> = (0..KEY_COUNT)
            .map(|_| gen_keypair_from_rng(&mut rng))
            .collect();

        (0..).map(
            move |i| match &keys.choose_multiple(&mut rng, 2).collect::<Vec<_>>()[..] {
                [(ref from, ref from_sk), (ref to, ..)] => RollbackTx::sign(from, to, i, from_sk),
                _ => unreachable!(),
            },
        )
    }
}

/// Writes transactions to the pool and returns their hashes.
fn prepare_txs(
    blockchain: &mut Blockchain,
    transactions: Vec<Signed<RawTransaction>>,
) -> Vec<Hash> {
    let mut fork = blockchain.fork();

    let tx_hashes = {
        let mut schema = Schema::new(&mut fork);

        // In the case of the block within `Bencher::iter()`, some transactions
        // may already be present in the pool. We don't particularly care about this.
        transactions
            .into_iter()
            .map(|tx| {
                schema.add_transaction_into_pool(tx.clone());
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

    assert!(tx_hashes
        .iter()
        .all(|hash| schema.transactions_pool().contains(&hash)
            && !schema.transactions_locations().contains(&hash)));
    assert_eq!(tx_hashes.len() as u64, schema.transactions_pool_len());
}

fn prepare_blockchain(
    blockchain: &mut Blockchain,
    generator: impl Iterator<Item = Signed<RawTransaction>>,
    blockchain_height: usize,
    txs_in_block: usize,
) {
    let transactions: Vec<_> = generator.take(txs_in_block * blockchain_height).collect();

    for i in 0..blockchain_height {
        let start = txs_in_block * i;
        let end = txs_in_block * (i + 1);
        let tx_hashes = prepare_txs(blockchain, transactions[start..end].to_vec());
        assert_transactions_in_pool(blockchain, &tx_hashes);

        let (block_hash, patch) = execute_block(blockchain, i as u64, &tx_hashes);
        // We make use of the fact that `Blockchain::commit()` doesn't check
        // precommits in any way (they are checked beforehand by the consensus algorithm).
        blockchain
            .commit(&patch, block_hash, iter::empty())
            .unwrap();
    }
}

fn execute_block_rocksdb(
    criterion: &mut Criterion,
    bench_name: &'static str,
    service: Box<dyn Service>,
    mut tx_generator: impl Iterator<Item = Signed<RawTransaction>>,
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

    let tx_hashes = prepare_txs(&mut blockchain, txs.clone());
    assert_transactions_in_pool(&blockchain, &tx_hashes);

    // Because execute_block is not really "micro benchmark"
    // executing it as regular benches, with 100 samples,
    // lead to relatively big testing time.
    // That's why the number of samples was decreased in each test.
    criterion.bench(
        bench_name,
        ParameterizedBenchmark::new(
            "transactions",
            move |bencher, &&txs_in_block| {
                let height: u64 = blockchain.last_block().height().next().into();
                bencher.iter(|| {
                    execute_block(&blockchain, height, &tx_hashes[..txs_in_block]);
                });
            },
            TXS_IN_BLOCK,
        )
        .sample_size(50)
        .throughput(|&&txs_in_block| Throughput::Elements(txs_in_block as u32)),
    );
}

pub fn bench_block(criterion: &mut Criterion) {
    use log::{self, LevelFilter};
    use std::panic;

    log::set_max_level(LevelFilter::Off);

    execute_block_rocksdb(
        criterion,
        "block/timestamping",
        timestamping::Timestamping.into(),
        timestamping::transactions(XorShiftRng::from_seed([2; 16])),
    );

    // We expect lots of panics here, so we switch their reporting off.
    let panic_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| ()));
    execute_block_rocksdb(
        criterion,
        "block/timestamping_panic",
        timestamping::Timestamping.into(),
        timestamping::panicking_transactions(XorShiftRng::from_seed([2; 16])),
    );
    panic::set_hook(panic_hook);

    execute_block_rocksdb(
        criterion,
        "block/cryptocurrency",
        cryptocurrency::Cryptocurrency.into(),
        cryptocurrency::provable_transactions(XorShiftRng::from_seed([3; 16])),
    );

    execute_block_rocksdb(
        criterion,
        "block/cryptocurrency_no_proofs",
        cryptocurrency::Cryptocurrency.into(),
        cryptocurrency::unprovable_transactions(XorShiftRng::from_seed([4; 16])),
    );

    execute_block_rocksdb(
        criterion,
        "block/cryptocurrency_rollback",
        cryptocurrency::Cryptocurrency.into(),
        cryptocurrency::rollback_transactions(XorShiftRng::from_seed([4; 16])),
    );
}
