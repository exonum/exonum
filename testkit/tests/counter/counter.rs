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

//! Sample counter service.
use exonum::{
    api, blockchain::{ExecutionError, ExecutionResult, Service, Transaction, TransactionSet},
    crypto::{Hash, PublicKey}, encoding, messages::{Message, RawTransaction},
    node::TransactionSend, storage::{Entry, Fork, Snapshot},
};

pub const SERVICE_ID: u16 = 1;

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
    pub CounterTransactions {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub tx_hash: Hash,
}

#[derive(Debug, Clone, Copy)]
struct CounterApi;

impl CounterApi {
    fn increment(
        state: &api::ServiceApiState,
        transaction: TxIncrement,
    ) -> api::Result<TransactionResponse> {
        trace!("received increment tx");

        let transaction: Box<Transaction> = Box::new(transaction);
        let tx_hash = transaction.hash();
        state.sender().send(transaction)?;
        Ok(TransactionResponse { tx_hash })
    }

    fn count(state: &api::ServiceApiState, _query: ()) -> api::Result<u64> {
        let snapshot = state.snapshot();
        let schema = CounterSchema::new(&snapshot);
        Ok(schema.count().unwrap_or_default())
    }

    fn reset(
        state: &api::ServiceApiState,
        transaction: TxReset,
    ) -> api::Result<TransactionResponse> {
        trace!("received reset tx");

        let transaction: Box<Transaction> = Box::new(transaction);
        let tx_hash = transaction.hash();
        state.sender().send(transaction)?;
        Ok(TransactionResponse { tx_hash })
    }

    fn wire(builder: &mut api::ServiceApiBuilder) {
        builder
            .private_scope()
            .endpoint("count", Self::count)
            .endpoint_mut("reset", Self::reset);
        builder
            .public_scope()
            .endpoint("count", Self::count)
            .endpoint_mut("count", Self::increment);
    }
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

    /// Implement a method to deserialize transactions coming to the node.
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let tx = CounterTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    fn wire_api(&self, builder: &mut api::ServiceApiBuilder) {
        CounterApi::wire(builder)
    }
}
