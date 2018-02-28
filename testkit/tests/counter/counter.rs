// Copyright 2017 The Exonum Team
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

//! Sample counter service.

use exonum::blockchain::{ApiContext, Service, Transaction, TransactionSet, ExecutionError,
                         ExecutionResult};
use exonum::messages::{Message, RawTransaction};
use exonum::storage::{Entry, Fork, Snapshot};
use exonum::crypto::{Hash, PublicKey};
use exonum::encoding;
use exonum::api::iron::{self, IronAdapter};
use exonum::api::ext::{ApiError, Endpoint, ReadContext, Spec, ServiceApi};

const SERVICE_ID: u16 = 1;

// "correct horse battery staple" brainwallet pubkey in Ed25519 with SHA-256 digest
pub const ADMIN_KEY: &str = "506f27b1b4c2403f2602d663a059b0262afd6a5bcda95a08dd96a4614a89f1b0";

// // // // Schema // // // //

pub struct CounterSchema<T> {
    view: T,
}

impl<T: AsRef<Snapshot>> CounterSchema<T> {
    pub fn new(view: T) -> Self {
        CounterSchema { view }
    }

    fn entry(&self) -> Entry<&Snapshot, u64> {
        Entry::new("counter.count", self.view.as_ref())
    }

    pub fn count(&self) -> Option<u64> {
        self.entry().get()
    }
}

impl<'a> CounterSchema<&'a mut Fork> {
    fn entry_mut(&mut self) -> Entry<&mut Fork, u64> {
        Entry::new("counter.count", self.view)
    }

    fn inc_count(&mut self, inc: u64) -> u64 {
        let count = self.count().unwrap_or(0) + inc;
        self.entry_mut().set(count);
        count
    }

    fn set_count(&mut self, count: u64) {
        self.entry_mut().set(count);
    }
}

// // // // Transactions // // // //

transactions! {
    CounterTransactions {
        const SERVICE_ID = SERVICE_ID;

        struct TxIncrement {
            author: &PublicKey,
            by: u64,
        }

        struct TxReset {
            author: &PublicKey,
        }
    }
}

impl Transaction for TxIncrement {
    fn verify(&self) -> bool {
        self.verify_signature(self.author())
    }

    // This method purposely does not check counter overflow in order to test
    // behavior of panicking transactions.
    fn execute(&self, fork: &mut Fork) -> ExecutionResult {
        if self.by() == 0 {
            Err(ExecutionError::with_description(
                0,
                "Adding zero does nothing!".to_string(),
            ))?;
        }

        let mut schema = CounterSchema::new(fork);
        schema.inc_count(self.by());
        Ok(())
    }
}

impl TxReset {
    pub fn verify_author(&self) -> bool {
        use exonum::encoding::serialize::FromHex;
        *self.author() == PublicKey::from_hex(ADMIN_KEY).unwrap()
    }
}

impl Transaction for TxReset {
    fn verify(&self) -> bool {
        self.verify_author() && self.verify_signature(self.author())
    }

    fn execute(&self, fork: &mut Fork) -> ExecutionResult {
        let mut schema = CounterSchema::new(fork);
        schema.set_count(0);
        Ok(())
    }
}

// // // // API // // // //

const COUNT_SPEC: Spec = Spec { id: "count" };

fn count(ctx: &ReadContext, _: ()) -> Result<u64, ApiError> {
    let counter = CounterSchema::new(ctx.snapshot()).count().unwrap_or(0);
    Ok(counter)
}

// // // // Service // // // //

pub struct CounterService;

impl Service for CounterService {
    fn service_name(&self) -> &str {
        "counter"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let tx = CounterTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<iron::Handler>> {
        let mut api = ServiceApi::new();
        api.insert(COUNT_SPEC, Endpoint::new(count));
        api.set_transactions::<CounterTransactions>();
        Some(IronAdapter::new(ctx.clone()).create_handler(api))
    }

    fn private_api_handler(&self, ctx: &ApiContext) -> Option<Box<iron::Handler>> {
        let mut api = ServiceApi::new();
        api.insert(COUNT_SPEC, Endpoint::new(count));
        api.set_transactions::<TxReset>();
        Some(IronAdapter::new(ctx.clone()).create_handler(api))
    }
}
