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

use criterion::{BatchSize, Bencher, Criterion};
use rand::{rngs::StdRng, Rng, SeedableRng};

use exonum::{
    blockchain::{
        config::GenesisConfigBuilder, ApiSender, Blockchain, BlockchainBuilder, ConsensusConfig,
        TxCheckCache,
    },
    crypto::KeyPair,
    merkledb::{Snapshot, TemporaryDB},
    messages::{AnyTx, Verified},
    runtime::{
        migrations::{InitMigrationError, MigrationScript},
        oneshot::Receiver,
        versioning::Version,
        ArtifactId, CallInfo, ExecutionContext, ExecutionError, InstanceId, InstanceState, Mailbox,
        Runtime, WellKnownRuntime,
    },
};

#[derive(Debug)]
struct DummyRuntime;

impl Runtime for DummyRuntime {
    fn deploy_artifact(&mut self, _artifact: ArtifactId, _deploy_spec: Vec<u8>) -> Receiver {
        Receiver::with_result(Ok(()))
    }

    fn is_artifact_deployed(&self, _artifact: &ArtifactId) -> bool {
        true
    }

    fn initiate_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn initiate_resuming_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn update_service_status(&mut self, _snapshot: &dyn Snapshot, _state: &InstanceState) {
        // Do nothing.
    }

    fn migrate(
        &self,
        _new_artifact: &ArtifactId,
        _data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        unimplemented!()
    }

    fn execute(
        &self,
        _context: ExecutionContext<'_>,
        _method_id: u32,
        _arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn before_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {
        // Do nothing.
    }
}

impl WellKnownRuntime for DummyRuntime {
    const ID: u32 = 255;
}

fn prepare_blockchain() -> Blockchain {
    const SERVICE_ID: InstanceId = 100;

    let (consensus_config, keys) = ConsensusConfig::for_tests(1);
    let artifact = ArtifactId::new(DummyRuntime::ID, "ts", Version::new(1, 0, 0)).unwrap();
    let service = artifact.clone().into_default_instance(SERVICE_ID, "ts");

    let blockchain = Blockchain::new(TemporaryDB::new(), keys.service, ApiSender::closed());
    let genesis_config = GenesisConfigBuilder::with_consensus_config(consensus_config)
        .with_artifact(artifact)
        .with_instance(service)
        .build();
    BlockchainBuilder::new(blockchain)
        .with_genesis_config(genesis_config)
        .with_runtime(DummyRuntime)
        .build()
        .immutable_view()
}

fn prepare_transactions(count: usize) -> Vec<Verified<AnyTx>> {
    const RNG_SEED: u64 = 123_456_789;

    let mut rng = StdRng::seed_from_u64(RNG_SEED);
    (0..count)
        .map(|_| {
            let mut payload = [0_u8; 64];
            rng.fill(&mut payload[..]);
            let payload = AnyTx::new(CallInfo::new(100, 0), payload.to_vec());
            payload.sign_with_keypair(&KeyPair::random())
        })
        .collect()
}

fn check_tx_no_cache(bencher: &mut Bencher) {
    let blockchain = prepare_blockchain();
    let transactions = prepare_transactions(128);
    let snapshot = blockchain.snapshot();
    bencher.iter(|| {
        assert!(transactions
            .iter()
            .all(|tx| Blockchain::check_tx(&snapshot, tx).is_ok()));
    })
}

fn check_tx_cache(bencher: &mut Bencher) {
    let blockchain = prepare_blockchain();
    let transactions = prepare_transactions(128);
    let snapshot = blockchain.snapshot();

    bencher.iter_batched(
        TxCheckCache::new,
        |mut cache| {
            assert!(transactions
                .iter()
                .all(|tx| Blockchain::check_tx_with_cache(&snapshot, tx, &mut cache).is_ok()));
        },
        BatchSize::SmallInput,
    )
}

pub fn bench_check_tx(c: &mut Criterion) {
    let mut group = c.benchmark_group("check_tx/single_service");
    group
        .bench_function("no_cache", check_tx_no_cache)
        .bench_function("cache", check_tx_cache);
    group.finish();
}
