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

// Suppress a warning in `transactions!` macro call:
#![cfg_attr(feature = "cargo-clippy", allow(redundant_field_names))]

use exonum::blockchain::{ExecutionError, ExecutionResult, Transaction};
use exonum::crypto::{CryptoHash, PublicKey};
use exonum::messages::Message;
use exonum::storage::Fork;
use exonum_time::schema::TimeSchema;

use TIMESTAMPING_SERVICE;
use schema::{Schema, Timestamp, TimestampEntry};

/// Error codes emitted by wallet transactions during execution.
#[derive(Debug, Fail)]
#[repr(u8)]
pub enum Error {
    /// Content hash already exists.
    #[fail(display = "Content hash already exists")]
    HashAlreadyExists = 0,
}

impl From<Error> for ExecutionError {
    fn from(value: Error) -> ExecutionError {
        let description = value.to_string();
        ExecutionError::with_description(value as u8, description)
    }
}

transactions! {
    pub TimeTransactions {
        const SERVICE_ID = TIMESTAMPING_SERVICE;

        /// A timestamp transaction.
        struct TxTimestamp {
            /// Public key of transaction.
            pub_key: &PublicKey,

            /// Timestamp content.
            content: Timestamp,
        }
    }
}

impl Transaction for TxTimestamp {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, fork: &mut Fork) -> ExecutionResult {
        let time = TimeSchema::new(&fork)
            .time()
            .get()
            .expect("Can't get the time");

        let content = self.content();
        let hash = content.content_hash();

        let mut schema = Schema::new(fork);
        if let Some(_entry) = schema.timestamps().get(hash) {
            Err(Error::HashAlreadyExists)?;
        }

        trace!("Timestamp added: {:?}", self);
        let entry = TimestampEntry::new(self.content(), &self.hash(), time);
        schema.add_timestamp(entry);
        Ok(())
    }
}
