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

//! Timestamping transactions.

// Workaround for `failure` see https://github.com/rust-lang-nursery/failure/issues/223 and
// ECR-1771 for the details.
#![allow(bare_trait_objects)]

use exonum::{
    blockchain::{ExecutionError, ExecutionResult, Transaction, TransactionContext},
    crypto::{PublicKey, SecretKey},
    messages::{Message, RawTransaction, Signed},
};
use exonum_time::schema::TimeSchema;

use super::proto;
use schema::{Schema, Timestamp, TimestampEntry};
use TIMESTAMPING_SERVICE;

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

/// Timestamping transaction.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TxTimestamp")]
pub struct TxTimestamp {
    /// Timestamp content.
    pub content: Timestamp,
}

/// Transaction group.
#[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
pub enum TimeTransactions {
    /// A timestamp transaction.
    TxTimestamp(TxTimestamp),
}

impl TxTimestamp {
    #[doc(hidden)]
    pub fn sign(author: &PublicKey, content: Timestamp, key: &SecretKey) -> Signed<RawTransaction> {
        Message::sign_transaction(Self { content }, TIMESTAMPING_SERVICE, *author, key)
    }
}

impl Transaction for TxTimestamp {
    fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
        let tx_hash = context.tx_hash();
        let time = TimeSchema::new(&context.fork())
            .time()
            .get()
            .expect("Can't get the time");

        let hash = &self.content.content_hash;

        let mut schema = Schema::new(context.fork());
        if let Some(_entry) = schema.timestamps().get(hash) {
            Err(Error::HashAlreadyExists)?;
        }

        trace!("Timestamp added: {:?}", self);
        let entry = TimestampEntry::new(self.content.clone(), &tx_hash, time);
        schema.add_timestamp(entry);
        Ok(())
    }
}
