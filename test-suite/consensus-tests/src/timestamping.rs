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

pub use crate::proto::TimestampTx;

use exonum::{
    blockchain::ExecutionError,
    crypto::{gen_keypair, Hash, PublicKey, SecretKey, HASH_SIZE},
    messages::Verified,
    runtime::{
        rust::{CallContext, Service, Transaction},
        AnyTx, InstanceId,
    },
};
use exonum_derive::*;
use exonum_merkledb::{access::AccessExt, BinaryValue};
use exonum_proto::impl_binary_value_for_pb_message;
use rand::{rngs::ThreadRng, thread_rng, RngCore};

pub const DATA_SIZE: usize = 64;

#[exonum_interface]
pub trait TimestampingInterface {
    fn timestamp(&self, context: CallContext<'_>, arg: TimestampTx) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("TimestampingInterface"))]
#[service_factory(
    artifact_name = "timestamping",
    artifact_version = "0.1.0",
    proto_sources = "crate::proto"
)]
pub struct TimestampingService;

impl TimestampingInterface for TimestampingService {
    fn timestamp(
        &self,
        _context: CallContext<'_>,
        _arg: TimestampTx,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }
}

impl Service for TimestampingService {
    fn initialize(&self, context: CallContext<'_>, _params: Vec<u8>) -> Result<(), ExecutionError> {
        context
            .service_data()
            .get_proof_entry("first")
            .set(Hash::new([127; HASH_SIZE]));
        context
            .service_data()
            .get_proof_entry("second")
            .set(Hash::new([128; HASH_SIZE]));
        Ok(())
    }
}

impl TimestampingService {
    pub const ID: InstanceId = 3;
}

impl_binary_value_for_pb_message! { TimestampTx }

#[derive(Debug)]
pub struct TimestampingTxGenerator {
    rand: ThreadRng,
    data_size: usize,
    public_key: PublicKey,
    secret_key: SecretKey,
    instance_id: InstanceId,
}

impl TimestampingTxGenerator {
    pub fn new(data_size: usize) -> Self {
        let keypair = gen_keypair();
        TimestampingTxGenerator::with_keypair(data_size, keypair)
    }

    /// Creates a generator of transactions for a service not instantiated on the blockchain.
    pub fn for_incorrect_service(data_size: usize) -> Self {
        let mut this = Self::new(data_size);
        this.instance_id += 1;
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
            instance_id: TimestampingService::ID,
        }
    }
}

impl Iterator for TimestampingTxGenerator {
    type Item = Verified<AnyTx>;

    fn next(&mut self) -> Option<Verified<AnyTx>> {
        let mut data = vec![0; self.data_size];
        self.rand.fill_bytes(&mut data);
        let mut tx = TimestampTx::new();
        tx.set_data(data);
        Some(tx.sign(self.instance_id, self.public_key, &self.secret_key))
    }
}
