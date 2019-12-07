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

pub use crate::proto::schema::tests::TimestampTx;

use exonum_merkledb::{BinaryValue, Snapshot};
use exonum_proto::impl_binary_value_for_pb_message;
use rand::{rngs::ThreadRng, thread_rng, RngCore};

use crate::{
    blockchain::ExecutionError,
    crypto::{gen_keypair, Hash, PublicKey, SecretKey, HASH_SIZE},
    messages::Verified,
    runtime::{
        rust::{CallContext, Service, TxStub},
        AnyTx, BlockchainData, InstanceId,
    },
};

pub const DATA_SIZE: usize = 64;

#[exonum_interface(crate = "crate")]
pub trait Timestamping {
    fn timestamp(&mut self, arg: TimestampTx) -> _;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate", implements("ServeTimestamping"))]
#[service_factory(
    crate = "crate",
    artifact_name = "timestamping",
    artifact_version = "0.1.0",
    proto_sources = "crate::proto::schema"
)]
pub struct TimestampingService;

impl ServeTimestamping for TimestampingService {
    fn timestamp(&self, _cx: CallContext<'_>, _arg: TimestampTx) -> Result<(), ExecutionError> {
        Ok(())
    }
}

impl Service for TimestampingService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![Hash::new([127; HASH_SIZE]), Hash::new([128; HASH_SIZE])]
    }
}

impl TimestampingService {
    pub const ID: InstanceId = 3;
}

impl_binary_value_for_pb_message! { TimestampTx }

pub struct TimestampingTxGenerator {
    rand: ThreadRng,
    data_size: usize,
    public_key: PublicKey,
    secret_key: SecretKey,
    tx_creator: TxStub,
}

impl TimestampingTxGenerator {
    pub fn new(data_size: usize) -> Self {
        let keypair = gen_keypair();
        TimestampingTxGenerator::with_keypair(data_size, keypair)
    }

    /// Creates a generator of transactions for a service not instantiated on the blockchain.
    pub fn for_incorrect_service(data_size: usize) -> Self {
        let mut this = Self::new(data_size);
        this.tx_creator = TxStub(TimestampingService::ID + 1);
        this
    }

    pub fn with_keypair(
        data_size: usize,
        keypair: (PublicKey, SecretKey),
    ) -> TimestampingTxGenerator {
        let rand = thread_rng();

        TimestampingTxGenerator {
            rand,
            data_size,
            public_key: keypair.0,
            secret_key: keypair.1,
            tx_creator: TxStub(TimestampingService::ID),
        }
    }
}

impl Iterator for TimestampingTxGenerator {
    type Item = Verified<AnyTx>;

    fn next(&mut self) -> Option<Verified<AnyTx>> {
        let mut data = vec![0; self.data_size];
        self.rand.fill_bytes(&mut data);
        let mut timestamp = TimestampTx::new();
        timestamp.set_data(data);
        let tx = self
            .tx_creator
            .timestamp(timestamp)
            .sign(self.public_key, &self.secret_key);
        Some(tx)
    }
}
