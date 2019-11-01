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
    blockchain::{
        Blockchain, BlockchainMut, ConsensusConfig, InstanceCollection, Schema, ValidatorKeys,
    },
    crypto::{self, Hash, PublicKey, SecretKey},
    helpers::{Height, ValidatorId},
    messages::{AnyTx, Verified},
    node::ApiSender,
};
use exonum_merkledb::{Database, DbOptions, ObjectHash, Patch, RocksDB};
use futures::sync::mpsc;
use rand::{rngs::StdRng, Rng, SeedableRng};
use tempdir::TempDir;

use std::{collections::BTreeMap, iter, sync::Arc};

/// Number of transactions added to the blockchain before the bench begins.
const PREPARE_TRANSACTIONS: usize = 10_000;
/// Tested values for the number of transactions in an added block.
///
/// `PREPARE_TRANSACTIONS` should be divisible by all values.
const TXS_IN_BLOCK: &[usize] = &[10, 25, 50, 100];

fn gen_keypair_from_rng(rng: &mut StdRng) -> (PublicKey, SecretKey) {
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
    services: Vec<InstanceCollection>,
) -> BlockchainMut {
    let service_keypair = (PublicKey::zero(), SecretKey::zero());
    let consensus_keypair = crypto::gen_keypair();
    let genesis_config = ConsensusConfig {
        validator_keys: vec![ValidatorKeys {
            consensus_key: consensus_keypair.0,
            service_key: service_keypair.0,
        }],
        ..ConsensusConfig::default()
    };

    let api_sender = ApiSender::new(mpsc::channel(0).0);
    Blockchain::new(db, service_keypair, api_sender)
        .into_mut(genesis_config)
        .with_rust_runtime(mpsc::channel(0).0, services)
        .build()
        .unwrap()
}

fn execute_block(blockchain: &BlockchainMut, height: u64, txs: &[Hash]) -> (Hash, Patch) {
    blockchain.create_patch(
        ValidatorId::zero(),
        Height(height),
        txs,
        &mut BTreeMap::new(),
    )
}

mod timestamping {
    use exonum::{
        blockchain::{ExecutionError, InstanceCollection},
        crypto::Hash,
        messages::Verified,
        runtime::{
            rust::{CallContext, Service, Transaction},
            AnyTx, InstanceDescriptor, InstanceId,
        },
    };
    use exonum_merkledb::{ObjectHash, Snapshot};
    use exonum_proto::ProtobufConvert;
    use rand::rngs::StdRng;

    use super::gen_keypair_from_rng;
    use crate::proto;

    const TIMESTAMPING_SERVICE_ID: InstanceId = 254;

    #[exonum_service]
    pub trait TimestampingInterface {
        fn timestamp(&self, context: CallContext, arg: Tx) -> Result<(), ExecutionError>;

        fn timestamp_panic(
            &self,
            context: CallContext,
            arg: PanickingTx,
        ) -> Result<(), ExecutionError>;
    }

    #[derive(Debug, ServiceFactory)]
    #[exonum(
        artifact_name = "timestamping",
        proto_sources = "crate::proto",
        implements("TimestampingInterface")
    )]
    pub struct Timestamping;

    impl TimestampingInterface for Timestamping {
        fn timestamp(&self, _context: CallContext, _arg: Tx) -> Result<(), ExecutionError> {
            Ok(())
        }

        fn timestamp_panic(
            &self,
            _context: CallContext,
            _arg: PanickingTx,
        ) -> Result<(), ExecutionError> {
            panic!("panic text");
        }
    }

    impl Service for Timestamping {
        fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
            vec![]
        }
    }

    impl From<Timestamping> for InstanceCollection {
        fn from(t: Timestamping) -> Self {
            Self::new(t).with_instance(TIMESTAMPING_SERVICE_ID, "timestamping", ())
        }
    }

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, ObjectHash, BinaryValue)]
    #[protobuf_convert(source = "proto::TimestampTx")]
    pub struct Tx {
        data: Hash,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, ObjectHash, BinaryValue)]
    #[protobuf_convert(source = "proto::TimestampTx")]
    pub struct PanickingTx {
        data: Hash,
    }

    pub fn transactions(mut rng: StdRng) -> impl Iterator<Item = Verified<AnyTx>> {
        (0_u32..).map(move |i| {
            let (pub_key, sec_key) = gen_keypair_from_rng(&mut rng);
            Tx {
                data: i.object_hash(),
            }
            .sign(TIMESTAMPING_SERVICE_ID, pub_key, &sec_key)
        })
    }

    pub fn panicking_transactions(mut rng: StdRng) -> impl Iterator<Item = Verified<AnyTx>> {
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
    use exonum::{
        blockchain::{ExecutionError, InstanceCollection},
        crypto::{Hash, PublicKey},
        messages::Verified,
        runtime::{
            rust::{CallContext, Service, Transaction},
            AnyTx, ErrorKind, InstanceDescriptor, InstanceId,
        },
    };
    use exonum_merkledb::{AccessExt, Snapshot};
    use exonum_proto::ProtobufConvert;
    use rand::{rngs::StdRng, seq::SliceRandom};

    use super::gen_keypair_from_rng;
    use crate::proto;

    const CRYPTOCURRENCY_SERVICE_ID: InstanceId = 255;

    // Number of generated accounts.
    const KEY_COUNT: usize = super::PREPARE_TRANSACTIONS / 10;
    // Initial balance of each account.
    const INITIAL_BALANCE: u64 = 100;

    #[exonum_service]
    pub trait CryptocurrencyInterface {
        /// Transfers one unit of currency from `from` to `to`.
        fn transfer(&self, context: CallContext, arg: Tx) -> Result<(), ExecutionError>;
        /// Same as `Tx`, but without cryptographic proofs in `execute`.
        fn transfer_without_proof(
            &self,
            context: CallContext,
            arg: SimpleTx,
        ) -> Result<(), ExecutionError>;
        /// Same as `SimpleTx`, but signals an error 50% of the time.
        fn transfer_error_sometimes(
            &self,
            context: CallContext,
            arg: RollbackTx,
        ) -> Result<(), ExecutionError>;
    }

    #[derive(Debug, ServiceFactory)]
    #[exonum(
        artifact_name = "cryptocurrency",
        proto_sources = "crate::proto",
        implements("CryptocurrencyInterface")
    )]
    pub struct Cryptocurrency;

    impl CryptocurrencyInterface for Cryptocurrency {
        fn transfer(&self, context: CallContext, arg: Tx) -> Result<(), ExecutionError> {
            let from = context.caller().author().unwrap();

            let mut index = context.fork().ensure_proof_map("provable_balances");

            let from_balance = index.get(&from).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(&arg.to).unwrap_or(INITIAL_BALANCE);
            index.put(&from, from_balance - 1);
            index.put(&arg.to, to_balance + 1);

            Ok(())
        }

        fn transfer_without_proof(
            &self,
            context: CallContext,
            arg: SimpleTx,
        ) -> Result<(), ExecutionError> {
            let from = context.caller().author().unwrap();

            let mut index = context.fork().ensure_map("balances");

            let from_balance = index.get(&from).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(&arg.to).unwrap_or(INITIAL_BALANCE);
            index.put(&from, from_balance - 1);
            index.put(&arg.to, to_balance + 1);

            Ok(())
        }

        fn transfer_error_sometimes(
            &self,
            context: CallContext,
            arg: RollbackTx,
        ) -> Result<(), ExecutionError> {
            let from = context.caller().author().unwrap();
            let mut index = context.fork().ensure_map("balances");

            let from_balance = index.get(&from).unwrap_or(INITIAL_BALANCE);
            let to_balance = index.get(&arg.to).unwrap_or(INITIAL_BALANCE);
            index.put(&from, from_balance - 1);
            index.put(&arg.to, to_balance + 1);

            // We deliberately perform the check *after* reads/writes in order
            // to check efficiency of rolling the changes back.
            if arg.seed % 2 == 0 {
                Ok(())
            } else {
                Err(ExecutionError::new(ErrorKind::service(15), ""))
            }
        }
    }

    impl Service for Cryptocurrency {
        fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
            vec![]
        }
    }

    impl From<Cryptocurrency> for InstanceCollection {
        fn from(t: Cryptocurrency) -> Self {
            Self::new(t).with_instance(CRYPTOCURRENCY_SERVICE_ID, "cryptocurrency", ())
        }
    }

    /// Transfers one unit of currency from `from` to `to`.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, ObjectHash, BinaryValue)]
    #[protobuf_convert(source = "proto::CurrencyTx")]
    pub struct Tx {
        to: PublicKey,
        seed: u32,
    }

    /// Same as `Tx`, but without cryptographic proofs in `execute`.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, ObjectHash, BinaryValue)]
    #[protobuf_convert(source = "proto::CurrencyTx")]
    pub struct SimpleTx {
        to: PublicKey,
        seed: u32,
    }

    /// Same as `SimpleTx`, but signals an error 50% of the time.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, ObjectHash, BinaryValue)]
    #[protobuf_convert(source = "proto::CurrencyTx")]
    pub struct RollbackTx {
        to: PublicKey,
        seed: u32,
    }

    pub fn provable_transactions(mut rng: StdRng) -> impl Iterator<Item = Verified<AnyTx>> {
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

    pub fn unprovable_transactions(mut rng: StdRng) -> impl Iterator<Item = Verified<AnyTx>> {
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

    pub fn rollback_transactions(mut rng: StdRng) -> impl Iterator<Item = Verified<AnyTx>> {
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

mod foreign_interface_call {
    use exonum::{
        blockchain::{ExecutionError, InstanceCollection},
        crypto::Hash,
        merkledb::ObjectHash,
        messages::Verified,
        runtime::{
            rust::{CallContext, Interface, Service, Transaction},
            AnyTx, DispatcherError, InstanceDescriptor, InstanceId, MethodId,
        },
    };
    use exonum_merkledb::Snapshot;
    use exonum_proto::ProtobufConvert;
    use rand::rngs::StdRng;

    use super::gen_keypair_from_rng;
    use crate::proto;

    const SELF_INTERFACE_SERVICE_ID: InstanceId = 254;
    const FOREIGN_INTERFACE_SERVICE_ID: InstanceId = 255;

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, ObjectHash, BinaryValue)]
    #[protobuf_convert(source = "proto::TimestampTx")]
    pub struct SelfTx {
        data: Hash,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, ObjectHash, BinaryValue)]
    #[protobuf_convert(source = "proto::TimestampTx")]
    pub struct ForeignTx {
        data: Hash,
    }

    #[exonum_service]
    pub trait SelfInterface {
        fn timestamp(&self, context: CallContext, arg: SelfTx) -> Result<(), ExecutionError>;

        fn timestamp_foreign(
            &self,
            context: CallContext,
            arg: ForeignTx,
        ) -> Result<(), ExecutionError>;
    }

    pub trait ForeignInterface {
        fn timestamp(&self, context: CallContext, arg: SelfTx) -> Result<(), ExecutionError>;
    }

    impl Interface for dyn ForeignInterface {
        const INTERFACE_NAME: &'static str = "ForeignInterface";

        fn dispatch(
            &self,
            ctx: CallContext,
            method: MethodId,
            payload: &[u8],
        ) -> Result<(), ExecutionError> {
            match method {
                0u32 => {
                    let bytes = payload.into();
                    let arg: SelfTx = exonum_merkledb::BinaryValue::from_bytes(bytes)
                        .map_err(DispatcherError::malformed_arguments)?;
                    self.timestamp(ctx, arg)
                }
                _ => Err(DispatcherError::NoSuchMethod).map_err(From::from),
            }
        }
    }

    #[derive(Debug)]
    pub struct ForeignInterfaceClient<'a>(CallContext<'a>);

    impl<'a> ForeignInterfaceClient<'a> {
        fn timestamp(&mut self, arg: SelfTx) -> Result<(), ExecutionError> {
            self.0.call(ForeignInterface::INTERFACE_NAME, 0, arg)
        }
    }

    impl<'a> From<CallContext<'a>> for ForeignInterfaceClient<'a> {
        fn from(context: CallContext<'a>) -> Self {
            Self(context)
        }
    }

    #[exonum_service(interface = "Configure")]
    pub trait Configure {}

    #[exonum_service(interface = "Events")]
    pub trait Events {}

    #[exonum_service(interface = "ERC30Tokens")]
    pub trait ERC30Tokens {}

    #[derive(Debug, ServiceFactory)]
    #[exonum(
        artifact_name = "timestamping",
        proto_sources = "crate::proto",
        implements(
            "SelfInterface",
            "ForeignInterface",
            "Configure",
            "Events",
            "ERC30Tokens"
        )
    )]
    pub struct Timestamping;

    impl SelfInterface for Timestamping {
        fn timestamp(&self, _context: CallContext, _arg: SelfTx) -> Result<(), ExecutionError> {
            Ok(())
        }

        fn timestamp_foreign(
            &self,
            mut context: CallContext,
            arg: ForeignTx,
        ) -> Result<(), ExecutionError> {
            context
                .interface::<ForeignInterfaceClient>(FOREIGN_INTERFACE_SERVICE_ID)?
                .timestamp(SelfTx { data: arg.data })
        }
    }

    impl ForeignInterface for Timestamping {
        fn timestamp(&self, context: CallContext, _arg: SelfTx) -> Result<(), ExecutionError> {
            assert_eq!(
                context.caller().as_service().unwrap(),
                SELF_INTERFACE_SERVICE_ID
            );
            Ok(())
        }
    }

    impl Configure for Timestamping {}

    impl Events for Timestamping {}

    impl ERC30Tokens for Timestamping {}

    impl Service for Timestamping {
        fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
            vec![]
        }
    }

    impl From<Timestamping> for InstanceCollection {
        fn from(t: Timestamping) -> Self {
            Self::new(t)
                .with_instance(SELF_INTERFACE_SERVICE_ID, "timestamping", Vec::default())
                .with_instance(
                    FOREIGN_INTERFACE_SERVICE_ID,
                    "timestamping-foreign",
                    Vec::default(),
                )
        }
    }

    pub fn self_transactions(mut rng: StdRng) -> impl Iterator<Item = Verified<AnyTx>> {
        (0_u32..).map(move |i| {
            let (pub_key, sec_key) = gen_keypair_from_rng(&mut rng);
            SelfTx {
                data: i.object_hash(),
            }
            .sign(SELF_INTERFACE_SERVICE_ID, pub_key, &sec_key)
        })
    }

    pub fn foreign_transactions(mut rng: StdRng) -> impl Iterator<Item = Verified<AnyTx>> {
        (0_u32..).map(move |i| {
            let (pub_key, sec_key) = gen_keypair_from_rng(&mut rng);
            ForeignTx {
                data: i.object_hash(),
            }
            .sign(SELF_INTERFACE_SERVICE_ID, pub_key, &sec_key)
        })
    }
}

/// Writes transactions to the pool and returns their hashes.
fn prepare_txs(blockchain: &mut BlockchainMut, transactions: Vec<Verified<AnyTx>>) -> Vec<Hash> {
    let fork = blockchain.fork();
    let mut schema = Schema::new(&fork);

    // In the case of the block within `Bencher::iter()`, some transactions
    // may already be present in the pool. We don't particularly care about this.
    let tx_hashes = transactions
        .into_iter()
        .map(|tx| {
            let hash = tx.object_hash();
            schema.add_transaction_into_pool(tx);
            hash
        })
        .collect();
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
    blockchain: &mut BlockchainMut,
    generator: impl Iterator<Item = Verified<AnyTx>>,
    blockchain_height: usize,
    txs_in_block: usize,
) {
    let transactions: Vec<_> = generator.take(txs_in_block * blockchain_height).collect();

    for i in 0..blockchain_height {
        let start = txs_in_block * i;
        let end = txs_in_block * (i + 1);
        let tx_hashes = prepare_txs(blockchain, transactions[start..end].to_vec());
        assert_transactions_in_pool(blockchain.as_ref(), &tx_hashes);

        let (block_hash, patch) = execute_block(blockchain, i as u64, &tx_hashes);
        // We make use of the fact that `Blockchain::commit()` doesn't check
        // precommits in any way (they are checked beforehand by the consensus algorithm).
        blockchain
            .commit(patch, block_hash, iter::empty(), &mut BTreeMap::new())
            .unwrap();
    }
}

fn execute_block_rocksdb(
    criterion: &mut Criterion,
    bench_name: &'static str,
    service: impl Into<InstanceCollection>,
    mut tx_generator: impl Iterator<Item = Verified<AnyTx>>,
) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    let mut blockchain = create_blockchain(db, vec![service.into()]);

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
    assert_transactions_in_pool(blockchain.as_ref(), &tx_hashes);

    // Because execute_block is not really "micro benchmark"
    // executing it as regular benches, with 100 samples,
    // lead to relatively big testing time.
    // That's why the number of samples was decreased in each test.
    criterion.bench(
        bench_name,
        ParameterizedBenchmark::new(
            "transactions",
            move |bencher, &&txs_in_block| {
                let height: u64 = blockchain.as_ref().last_block().height().next().into();
                bencher.iter(|| {
                    execute_block(&blockchain, height, &tx_hashes[..txs_in_block]);
                });
            },
            TXS_IN_BLOCK,
        )
        .sample_size(100)
        .throughput(|&&txs_in_block| Throughput::Elements(txs_in_block as u64)),
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
        timestamping::transactions(SeedableRng::from_seed([2; 32])),
    );

    // We expect lots of panics here, so we switch their reporting off.
    let panic_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| ()));
    execute_block_rocksdb(
        criterion,
        "block/timestamping_panic",
        timestamping::Timestamping,
        timestamping::panicking_transactions(SeedableRng::from_seed([2; 32])),
    );
    panic::set_hook(panic_hook);

    execute_block_rocksdb(
        criterion,
        "block/cryptocurrency",
        cryptocurrency::Cryptocurrency,
        cryptocurrency::provable_transactions(SeedableRng::from_seed([3; 32])),
    );

    execute_block_rocksdb(
        criterion,
        "block/cryptocurrency_no_proofs",
        cryptocurrency::Cryptocurrency,
        cryptocurrency::unprovable_transactions(SeedableRng::from_seed([4; 32])),
    );

    execute_block_rocksdb(
        criterion,
        "block/cryptocurrency_rollback",
        cryptocurrency::Cryptocurrency,
        cryptocurrency::rollback_transactions(SeedableRng::from_seed([4; 32])),
    );

    execute_block_rocksdb(
        criterion,
        "block/foreign_interface_call/self_tx",
        foreign_interface_call::Timestamping,
        foreign_interface_call::self_transactions(SeedableRng::from_seed([2; 32])),
    );

    execute_block_rocksdb(
        criterion,
        "block/foreign_interface_call/foreign_tx",
        foreign_interface_call::Timestamping,
        foreign_interface_call::foreign_transactions(SeedableRng::from_seed([2; 32])),
    );
}
