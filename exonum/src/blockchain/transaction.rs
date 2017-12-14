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

//! `Transaction` related types.

use std::borrow::Cow;

use serde_json::Value;

use messages::Message;
use storage::{Fork, StorageValue};
use crypto::Hash;

// TODO: Remove attribute when `const fn` becomes stable.
#[cfg_attr(feature="cargo-clippy", allow(cast_lossless))]
static MAX_RESERVED_VALUE: u16 = ::std::u8::MAX as u16;

/// A trait that describes transaction processing rules (a group of sequential operations
/// with the Exonum storage) for the given `Message`.
pub trait Transaction: Message + 'static {
    /// Verifies the transaction, which includes the message signature verification and other
    /// specific internal constraints. verify is intended to check the internal consistency of
    /// a transaction; it has no access to the blockchain state.
    /// If a transaction fails verify, it is considered incorrect and cannot be included into
    /// any correct block proposal. Incorrect transactions are never included into the blockchain.
    ///
    /// *This method should not use external data, that is, it must be a pure function.*
    fn verify(&self) -> bool;

    /// Takes the current blockchain state via `fork` and can modify it if certain conditions
    /// are met.
    ///
    /// # Notes
    ///
    /// - When programming `execute`, you should perform state-related checks before any changes
    /// to the state and return early if these checks fail.
    /// - If the execute method of a transaction raises a `panic`, the changes made by the
    /// transactions are discarded, but the transaction itself is still considered committed.
    /// - A transaction execution status (see `ExecutionStatus` for the details) is stored in the
    /// blockchain and can be accessed through api.
    /// - A transaction is considered failed if it made no changes to the storage, but its execution
    /// status can be set explicitly (see `ExecutionContext` for the details).
    fn execute(&self, context: &mut ExecutionContext);

    /// Returns the useful information about the transaction in the JSON format. The returned value
    /// is used to fill the [`TxInfo.content`] field in [the blockchain explorer][explorer].
    ///
    /// # Notes
    ///
    /// The default implementation returns `null`. For transactions defined with
    /// the [`message!`] macro, you may redefine `info()` as
    ///
    /// ```
    /// # #[macro_use] extern crate exonum;
    /// extern crate serde_json;
    /// # use exonum::blockchain::Transaction;
    /// # use exonum::blockchain::transaction::ExecutionContext;
    /// # use exonum::storage::Fork;
    ///
    /// message! {
    ///     struct MyTransaction {
    ///         // Transaction definition...
    /// #       const TYPE = 1;
    /// #       const ID = 1;
    /// #       const SIZE = 8;
    /// #       field foo: u64 [0 => 8]
    ///     }
    /// }
    ///
    /// impl Transaction for MyTransaction {
    ///     // Other methods...
    /// #   fn verify(&self) -> bool { true }
    /// #   fn execute(&self, _: &mut ExecutionContext) {}
    ///
    ///     fn info(&self) -> serde_json::Value {
    ///         serde_json::to_value(self).expect("Cannot serialize transaction to JSON")
    ///     }
    /// }
    /// # fn main() { }
    /// ```
    ///
    /// [`TxInfo.content`]: ../explorer/struct.TxInfo.html#structfield.content
    /// [explorer]: ../explorer/index.html
    /// [`message!`]: ../macro.message.html
    fn info(&self) -> Value {
        Value::Null
    }
}

/// Execution status of the transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutionStatus {
    /// Successful transaction execution.
    Succeeded,
    /// Panic occurred during transaction execution.
    Panic,
    /// General failure (unspecified reason).
    Failed,
    /// User defined execution status. Can have different meanings for different transactions and
    /// services.
    Custom(u8),
}

/// `Transaction`'s execution context.
#[derive(Debug)]
pub struct ExecutionContext<'a> {
    fork: &'a mut Fork,
    status: Option<ExecutionStatus>,
    changes_number: usize,
}

impl<'a> ExecutionContext<'a> {
    /// Creates a new `ExecutionContext` instance.
    pub fn new(fork: &'a mut Fork) -> Self {
        Self {
            changes_number: fork.patch().len(),
            fork,
            status: None,
        }
    }

    /// Returns execution status consuming `ExecutionContext`.
    pub fn into_status(self) -> ExecutionStatus {
        match self.status {
            Some(val) => val,
            None => {
                if self.fork.patch().len() > self.changes_number {
                    ExecutionStatus::Succeeded
                } else {
                    ExecutionStatus::Failed
                }
            }
        }
    }

    /// Returns reference to the `Fork`.
    pub fn fork(&mut self) -> &mut Fork {
        self.fork
    }

    /// Sets `Transaction`'s execution status. See `ExecutionStatus` for the details.
    pub fn set_execution_status(&mut self, status: ExecutionStatus) {
        self.status = Some(status);
    }
}

impl<'a, T: Transaction> From<T> for Box<Transaction + 'a> {
    fn from(tx: T) -> Self {
        Box::new(tx) as Box<Transaction>
    }
}

impl StorageValue for ExecutionStatus {
    fn hash(&self) -> Hash {
        u16::hash(&from_status(*self))
    }

    fn into_bytes(self) -> Vec<u8> {
        u16::into_bytes(from_status(self))
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        to_status(u16::from_bytes(value))
    }
}

fn from_status(status: ExecutionStatus) -> u16 {
    match status {
        ExecutionStatus::Succeeded => 0,
        ExecutionStatus::Panic => 1,
        ExecutionStatus::Failed => 2,
        ExecutionStatus::Custom(value) => u16::from(value) + MAX_RESERVED_VALUE + 1,
    }
}

fn to_status(value: u16) -> ExecutionStatus {
    match value {
        0 => ExecutionStatus::Succeeded,
        1 => ExecutionStatus::Panic,
        2 => ExecutionStatus::Failed,
        val => {
            assert!(
                val > MAX_RESERVED_VALUE,
                "Invalid ExecutionStatus value: {}",
                val
            );
            ExecutionStatus::Custom((val - MAX_RESERVED_VALUE - 1) as u8)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{Database, MemoryDB, ListIndex};

    #[test]
    fn execution_status_round_trip() {
        let statuses = [
            ExecutionStatus::Succeeded,
            ExecutionStatus::Panic,
            ExecutionStatus::Failed,
            ExecutionStatus::Custom(0),
            ExecutionStatus::Custom(1),
            ExecutionStatus::Custom(100),
            ExecutionStatus::Custom(255),
        ];

        for status in &statuses {
            let bytes = status.clone().into_bytes();
            let new_status = ExecutionStatus::from_bytes(Cow::Borrowed(&bytes));

            assert_eq!(*status, new_status);
        }
    }

    #[test]
    fn implicit_execution_status() {
        let db = MemoryDB::new();
        let mut fork = db.fork();

        {
            let context = ExecutionContext::new(&mut fork);
            assert_eq!(ExecutionStatus::Failed, context.into_status());
        }

        let mut context = ExecutionContext::new(&mut fork);
        {
            let mut index = ListIndex::new("test", context.fork());
            index.push(1u8);
        }
        assert_eq!(ExecutionStatus::Succeeded, context.into_status());
    }
}
