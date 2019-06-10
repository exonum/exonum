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
    blockchain::{Blockchain, GenesisConfig, Schema, ValidatorKeys},
    crypto::{self, Hash, PublicKey, SecretKey},
    helpers::{Height, ValidatorId},
    messages::{AnyTx, Signed},
    node::ApiSender,
    runtime::dispatcher::{BuiltinService, DispatcherBuilder},
};
use exonum_merkledb::{Database, DbOptions, ObjectHash, Patch, RocksDB};
use futures::sync::mpsc;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use tempdir::TempDir;

use std::{iter, sync::Arc};

/// Number of transactions added to the blockchain before the bench begins.
const PREPARE_TRANSACTIONS: usize = 10_000;
/// Tested values for the number of transactions in an added block.
///
/// `PREPARE_TRANSACTIONS` should be divisible by all values.
const TXS_IN_BLOCK: &[usize] = &[10, 25, 50, 100];

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

fn create_blockchain(
    db: impl Into<Arc<dyn Database>>,
    services: Vec<impl Into<BuiltinService>>,
) -> Blockchain {
    let dummy_channel = mpsc::unbounded();
    let service_keypair = (PublicKey::zero(), SecretKey::zero());

    let mut dispatcher = DispatcherBuilder::new(mpsc::channel(0).0);
    for service in services {
        dispatcher = dispatcher.with_builtin_service(service);
    }
    let dispatcher = dispatcher.finalize();

    let mut blockchain = Blockchain::with_dispatcher(
        db,
        dispatcher,
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
    use exonum::{
        blockchain::ExecutionResult,
        crypto::Hash,
        impl_service_dispatcher,
        messages::{AnyTx, ServiceInstanceId, Signed},
        runtime::dispatcher::BuiltinService,
        runtime::rust::{
            RustArtifactSpec, Service, ServiceFactory, Transaction, TransactionContext,
        },
    };
    use exonum_merkledb::ObjectHash;
    use rand::Rng;

    use super::gen_keypair_from_rng;
    use crate::proto;

    const TIMESTAMPING_SERVICE_ID: ServiceInstanceId = 254;

    #[service_interface]
    pub trait TimestampingInterface {
        fn timestamp(&self, context: TransactionContext, arg: Tx) -> ExecutionResult;

        fn timestamp_panic(&self, context: TransactionContext, arg: PanickingTx)
            -> ExecutionResult;
    }

    #[derive(Debug)]
    pub struct Timestamping;

    impl TimestampingInterface for Timestamping {
        fn timestamp(&self, _context: TransactionContext, _arg: Tx) -> ExecutionResult {
            Ok(())
        }
        fn timestamp_panic(
            &self,
            _context: TransactionContext,
            _arg: PanickingTx,
        ) -> ExecutionResult {
            panic!("panic text");
        }
    }

    impl_service_dispatcher!(Timestamping, TimestampingInterface);

    impl Service for Timestamping {}

    impl ServiceFactory for Timestamping {
        fn artifact(&self) -> RustArtifactSpec {
            "timestamping/0.0.1".parse().unwrap()
        }
        fn new_instance(&self) -> Box<dyn Service> {
            Box::new(Self)
        }
    }

    impl From<Timestamping> for BuiltinService {
        fn from(t: Timestamping) -> Self {
            Self {
                factory: Box::new(t),
                instance_id: TIMESTAMPING_SERVICE_ID,
                instance_name: "timestamping".into(),
            }
        }
    }

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::TimestampTx")]
    pub struct Tx {
        data: Hash,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::TimestampTx")]
    pub struct PanickingTx {
        data: Hash,
    }

    pub fn transactions(mut rng: impl Rng) -> impl Iterator<Item = Signed<AnyTx>> {
        (0_u32..).map(move |i| {
            let (pub_key, sec_key) = gen_keypair_from_rng(&mut rng);
            Tx {
                data: i.object_hash(),
            }
            .sign(TIMESTAMPING_SERVICE_ID, pub_key, &sec_key)
        })
    }

    pub fn panicking_transactions(mut rng: impl Rng) -> impl Iterator<Item = Signed<AnyTx>> {
        (0_u32..).map(move |i| {
            let (pub_key, sec_key) = gen_keypair_from_rng(&mut rng);
            PanickingTx {
                data: i.object_hash(),
            }
            .sign(TIMESTAMPING_SERVICE_ID, pub_key, &sec_key)
        })
    }
}

mod cryptocurrency {
    use super::gen_keypair_from_rng;
    use crate::proto;
    use exonum::{
        blockchain::{ExecutionError, ExecutionResult},
        crypto::PublicKey,
        impl_service_dispatcher,
        messages::{AnyTx, ServiceInstanceId, Signed},
        runtime::{
            dispatcher::BuiltinService,
            rust::{RustArtifactSpec, Service, ServiceFactory, Transaction, TransactionContext},
        },
    };
    use exonum_merkledb::{MapIndex, ProofMapIndex};
    use rand::{seq::SliceRandom, Rng};

    const CRYPTOCURRENCY_SERVICE_ID: ServiceInstanceId = 255;

    // Number of generated accounts.
    const KEY_COUNT: usize = super::PREPARE_TRANSACTIONS / 10;
    // Initial balance of each account.
    const INITIAL_BALANCE: u64 = 100;

    #[service_interface]
    pub trait CryptocurrencyInterface {
        /// Transfers one unit of currency from `from` to `to`.
        fn transfer(&self, context: TransactionContext, arg: Tx) -> ExecutionResult;
        /// Same as `Tx`, but without cryptographic proofs in `execute`.
        fn transfer_without_proof(
            &self,
            context: TransactionContext,
            arg: SimpleTx,
        ) -> ExecutionResult;
        /// Same as `SimpleTx`, but signals an error 50% of the time.
        fn transfer_error_sometimes(
            &self,
            context: TransactionContext,
            arg: RollbackTx,
        ) -> ExecutionResult;
    }

    #[derive(Debug)]
    pub struct Cryptocurrency;

    impl CryptocurrencyInterface for Cryptocurrency {
        fn transfer(&self, context: TransactionContext, arg: Tx) -> ExecutionResult {
            let from = context.author();
            let mut index = ProofMapIndex::new("provable_balances", context.fork());

            let from_balance = index.get(&from).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(&arg.to).unwrap_or(INITIAL_BALANCE);
            index.put(&from, from_balance - 1);
            index.put(&arg.to, to_balance + 1);

            Ok(())
        }

        fn transfer_without_proof(
            &self,
            context: TransactionContext,
            arg: SimpleTx,
        ) -> ExecutionResult {
            let from = context.author();

            let mut index = MapIndex::new("balances", context.fork());

            let from_balance = index.get(&from).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(&arg.to).unwrap_or(INITIAL_BALANCE);
            index.put(&from, from_balance - 1);
            index.put(&arg.to, to_balance + 1);

            Ok(())
        }

        fn transfer_error_sometimes(
            &self,
            context: TransactionContext,
            arg: RollbackTx,
        ) -> ExecutionResult {
            let from = context.author();

            let mut index = MapIndex::new("balances", context.fork());

            let from_balance = index.get(&from).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(&arg.to).unwrap_or(INITIAL_BALANCE);
            index.put(&from, from_balance - 1);
            index.put(&arg.to, to_balance + 1);

            // We deliberately perform the check *after* reads/writes in order
            // to check efficiency of rolling the changes back.
            if arg.seed % 2 == 0 {
                Ok(())
            } else {
                Err(ExecutionError::new(1))
            }
        }
    }

    impl_service_dispatcher!(Cryptocurrency, CryptocurrencyInterface);

    impl Service for Cryptocurrency {}

    impl ServiceFactory for Cryptocurrency {
        fn artifact(&self) -> RustArtifactSpec {
            "cryptocurrency/0.0.1".parse().unwrap()
        }
        fn new_instance(&self) -> Box<dyn Service> {
            Box::new(Self)
        }
    }

    impl From<Cryptocurrency> for BuiltinService {
        fn from(t: Cryptocurrency) -> Self {
            Self {
                factory: Box::new(t),
                instance_id: CRYPTOCURRENCY_SERVICE_ID,
                instance_name: "cryptocurrency".into(),
            }
        }
    }

    /// Transfers one unit of currency from `from` to `to`.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::CurrencyTx")]
    pub struct Tx {
        to: PublicKey,
        seed: u32,
    }

    /// Same as `Tx`, but without cryptographic proofs in `execute`.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::CurrencyTx")]
    pub struct SimpleTx {
        to: PublicKey,
        seed: u32,
    }

    /// Same as `SimpleTx`, but signals an error 50% of the time.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::CurrencyTx")]
    pub struct RollbackTx {
        to: PublicKey,
        seed: u32,
    }

    pub fn provable_transactions(mut rng: impl Rng) -> impl Iterator<Item = Signed<AnyTx>> {
        let keys: Vec<_> = (0..KEY_COUNT)
            .map(|_| gen_keypair_from_rng(&mut rng))
            .collect();

        (0..).map(
            move |seed| match &keys.choose_multiple(&mut rng, 2).collect::<Vec<_>>()[..] {
                [(ref from, ref from_sk), (ref to, ..)] => {
                    Tx { to: *to, seed }.sign(CRYPTOCURRENCY_SERVICE_ID, *from, &from_sk)
                }
                _ => unreachable!(),
            },
        )
    }

    pub fn unprovable_transactions(mut rng: impl Rng) -> impl Iterator<Item = Signed<AnyTx>> {
        let keys: Vec<_> = (0..KEY_COUNT)
            .map(|_| gen_keypair_from_rng(&mut rng))
            .collect();

        (0..).map(
            move |seed| match &keys.choose_multiple(&mut rng, 2).collect::<Vec<_>>()[..] {
                [(ref from, ref from_sk), (ref to, ..)] => {
                    SimpleTx { to: *to, seed }.sign(CRYPTOCURRENCY_SERVICE_ID, *from, &from_sk)
                }
                _ => unreachable!(),
            },
        )
    }

    pub fn rollback_transactions(mut rng: impl Rng) -> impl Iterator<Item = Signed<AnyTx>> {
        let keys: Vec<_> = (0..KEY_COUNT)
            .map(|_| gen_keypair_from_rng(&mut rng))
            .collect();

        (0..).map(
            move |seed| match &keys.choose_multiple(&mut rng, 2).collect::<Vec<_>>()[..] {
                [(ref from, ref from_sk), (ref to, ..)] => {
                    RollbackTx { to: *to, seed }.sign(CRYPTOCURRENCY_SERVICE_ID, *from, &from_sk)
                }
                _ => unreachable!(),
            },
        )
    }
}

/// Writes transactions to the pool and returns their hashes.
fn prepare_txs(blockchain: &mut Blockchain, transactions: Vec<Signed<AnyTx>>) -> Vec<Hash> {
    let fork = blockchain.fork();

    let tx_hashes = {
        let mut schema = Schema::new(&fork);

        // In the case of the block within `Bencher::iter()`, some transactions
        // may already be present in the pool. We don't particularly care about this.
        transactions
            .into_iter()
            .map(|tx| {
                let hash = tx.object_hash();
                schema.add_transaction_into_pool(tx);
                hash
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
    generator: impl Iterator<Item = Signed<AnyTx>>,
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
    service: impl Into<BuiltinService>,
    mut tx_generator: impl Iterator<Item = Signed<AnyTx>>,
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

    let tx_hashes = prepare_txs(&mut blockchain, txs);
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
    use log::LevelFilter;
    use std::panic;

    log::set_max_level(LevelFilter::Off);

    execute_block_rocksdb(
        criterion,
        "block/timestamping",
        timestamping::Timestamping,
        timestamping::transactions(XorShiftRng::from_seed([2; 16])),
    );

    execute_block_rocksdb(
        criterion,
        "block/timestamping_panic",
        timestamping::Timestamping,
        timestamping::panicking_transactions(XorShiftRng::from_seed([2; 16])),
    );

    execute_block_rocksdb(
        criterion,
        "block/cryptocurrency",
        cryptocurrency::Cryptocurrency,
        cryptocurrency::provable_transactions(XorShiftRng::from_seed([3; 16])),
    );

    execute_block_rocksdb(
        criterion,
        "block/cryptocurrency_no_proofs",
        cryptocurrency::Cryptocurrency,
        cryptocurrency::unprovable_transactions(XorShiftRng::from_seed([4; 16])),
    );

    execute_block_rocksdb(
        criterion,
        "block/cryptocurrency_rollback",
        cryptocurrency::Cryptocurrency,
        cryptocurrency::rollback_transactions(XorShiftRng::from_seed([4; 16])),
    );
}
