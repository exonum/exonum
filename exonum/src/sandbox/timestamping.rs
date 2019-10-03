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
use rand::{rngs::ThreadRng, thread_rng, RngCore};

use crate::{
    blockchain::ExecutionError,
    crypto::{gen_keypair, Hash, PublicKey, SecretKey, HASH_SIZE},
    messages::Verified,
    runtime::{
        rust::{Service, Transaction, TransactionContext},
        AnyTx, InstanceDescriptor, InstanceId,
    },
};

pub const DATA_SIZE: usize = 64;

#[exonum_service(crate = "crate")]
pub trait TimestampingInterface {
    fn timestamp(
        &self,
        context: TransactionContext,
        arg: TimestampTx,
    ) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    crate = "crate",
    artifact_name = "timestamping",
    artifact_version = "0.1.0",
    proto_sources = "crate::proto::schema",
    implements("TimestampingInterface")
)]
pub struct TimestampingService;

impl TimestampingInterface for TimestampingService {
    fn timestamp(
        &self,
        _context: TransactionContext,
        _arg: TimestampTx,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }
}

impl Service for TimestampingService {
    fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
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
}

impl TimestampingTxGenerator {
    pub fn new(data_size: usize) -> TimestampingTxGenerator {
        let keypair = gen_keypair();
        TimestampingTxGenerator::with_keypair(data_size, keypair)
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
        }
    }
}

impl Iterator for TimestampingTxGenerator {
    type Item = Verified<AnyTx>;

    fn next(&mut self) -> Option<Verified<AnyTx>> {
        let mut data = vec![0; self.data_size];
        self.rand.fill_bytes(&mut data);
        let mut buf = TimestampTx::new();
        buf.set_data(data);
        Some(buf.sign(TimestampingService::ID, self.public_key, &self.secret_key))
    }
}
