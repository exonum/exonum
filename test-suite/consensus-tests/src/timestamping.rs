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

use exonum::{
    crypto::{gen_keypair, Hash, PublicKey, SecretKey, HASH_SIZE},
    messages::Verified,
    runtime::{
        rust::{CallContext, DefaultInstance, Service},
        AnyTx, ExecutionError, InstanceId,
    },
};
use exonum_derive::*;
use exonum_merkledb::access::AccessExt;
use rand::{rngs::ThreadRng, thread_rng, RngCore};

pub const DATA_SIZE: usize = 64;

#[exonum_interface]
pub trait Timestamping<Ctx> {
    type Output;
    fn timestamp(&self, ctx: Ctx, arg: Vec<u8>) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("Timestamping"))]
#[service_factory(artifact_name = "timestamping", artifact_version = "0.1.0")]
pub struct TimestampingService;

impl Timestamping<CallContext<'_>> for TimestampingService {
    type Output = Result<(), ExecutionError>;

    fn timestamp(&self, _ctx: CallContext<'_>, _arg: Vec<u8>) -> Self::Output {
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

/// Generator of timestamping transactions.
#[derive(Debug)]
pub struct TimestampingTxGenerator {
    rand: ThreadRng,
    data_size: usize,
    keypair: (PublicKey, SecretKey),
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
            keypair,
            instance_id: TimestampingService::ID,
        }
    }
}

impl Iterator for TimestampingTxGenerator {
    type Item = Verified<AnyTx>;

    fn next(&mut self) -> Option<Verified<AnyTx>> {
        let mut data = vec![0; self.data_size];
        self.rand.fill_bytes(&mut data);
        Some(self.keypair.timestamp(self.instance_id, data))
    }
}

impl DefaultInstance for TimestampingService {
    const INSTANCE_ID: InstanceId = TimestampingService::ID;
    const INSTANCE_NAME: &'static str = "timestamping";
}
