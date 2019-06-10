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

use exonum_merkledb::{impl_binary_value_for_message, BinaryValue, Snapshot};
use protobuf::Message as PbMessage;
use rand::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;
use semver::Version;

use std::borrow::Cow;

use crate::{
    blockchain::ExecutionResult,
    crypto::{gen_keypair, Hash, PublicKey, SecretKey, HASH_SIZE},
    messages::{AnyTx, ServiceInstanceId, Signed},
    runtime::{
        dispatcher::BuiltinService,
        rust::{
            RustArtifactSpec, Service, ServiceDescriptor, ServiceFactory, Transaction,
            TransactionContext,
        },
    },
};

pub const DATA_SIZE: usize = 64;

#[service_interface(exonum(crate = "crate"))]
pub trait TimestampingInterface {
    fn timestamp(&self, context: TransactionContext, arg: TimestampTx) -> ExecutionResult;
}

#[derive(Debug)]
pub struct TimestampingService;

impl_service_dispatcher!(TimestampingService, TimestampingInterface);

impl TimestampingInterface for TimestampingService {
    fn timestamp(&self, _context: TransactionContext, _arg: TimestampTx) -> ExecutionResult {
        Ok(())
    }
}

impl Service for TimestampingService {
    fn state_hash(&self, _descriptor: ServiceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![Hash::new([127; HASH_SIZE]), Hash::new([128; HASH_SIZE])]
    }
}

impl ServiceFactory for TimestampingService {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec {
            name: "timestamping".into(),
            version: Version::new(0, 1, 0),
        }
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

impl TimestampingService {
    pub const ID: ServiceInstanceId = 1;
}

impl From<TimestampingService> for BuiltinService {
    fn from(factory: TimestampingService) -> Self {
        Self {
            factory: factory.into(),
            instance_id: TimestampingService::ID,
            instance_name: "timestamping".into(),
        }
    }
}

impl_binary_value_for_message! { TimestampTx }

pub struct TimestampingTxGenerator {
    rand: XorShiftRng,
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
        let rand = XorShiftRng::from_seed([9; 16]);

        TimestampingTxGenerator {
            rand,
            data_size,
            public_key: keypair.0,
            secret_key: keypair.1,
        }
    }
}

impl Iterator for TimestampingTxGenerator {
    type Item = Signed<AnyTx>;

    fn next(&mut self) -> Option<Signed<AnyTx>> {
        let mut data = vec![0; self.data_size];
        self.rand.fill_bytes(&mut data);
        let mut buf = TimestampTx::new();
        buf.set_data(data);
        Some(buf.sign(TimestampingService::ID, self.public_key, &self.secret_key))
    }
}
