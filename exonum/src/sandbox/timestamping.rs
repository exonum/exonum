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

use rand::{Rng, SeedableRng, XorShiftRng};

use blockchain::{ExecutionResult, Service, Transaction, TransactionContext, TransactionSet};
use crypto::{gen_keypair, Hash, PublicKey, SecretKey};
use encoding::Error as MessageError;
use messages::{Message, RawTransaction};
use storage::Snapshot;

pub const TIMESTAMPING_SERVICE: u16 = 129;

transactions! {
    TimestampingTransactions {

        struct TimestampTx {
            data: &[u8],
        }
    }
}

impl Transaction for TimestampTx {
    fn verify(&self) -> bool {
        true
    }

    fn execute<'a>(&self, _: TransactionContext<'a>) -> ExecutionResult {
        Ok(())
    }
}

#[derive(Default)]
pub struct TimestampingService {}

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
        let rand = XorShiftRng::from_seed([192, 168, 56, 1]);

        TimestampingTxGenerator {
            rand,
            data_size,
            public_key: keypair.0,
            secret_key: keypair.1,
        }
    }
}

impl Iterator for TimestampingTxGenerator {
    type Item = Message<RawTransaction>;

    fn next(&mut self) -> Option<Message<RawTransaction>> {
        let mut data = vec![0; self.data_size];
        self.rand.fill_bytes(&mut data);
        let buf = TimestampTx::new(&self.public_key, &data, &self.secret_key).clone();
        Some(Message::sign_tx(
            buf,
            TIMESTAMPING_SERVICE,
            (self.public_key, &self.secret_key),
        ))
    }
}

impl TimestampingService {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Service for TimestampingService {
    fn service_name(&self) -> &str {
        "sandbox_timestamping"
    }

    fn service_id(&self) -> u16 {
        TIMESTAMPING_SERVICE
    }

    fn state_hash(&self, _: &dyn Snapshot) -> Vec<Hash> {
        vec![Hash::new([127; 32]), Hash::new([128; 32])]
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, MessageError> {
        let tx = TimestampingTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }
}
