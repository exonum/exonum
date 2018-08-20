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

//! A special service which generates transactions on `after_commit` events.

use exonum::{
    blockchain::{ExecutionResult, Service, ServiceContext, Transaction, TransactionSet},
    crypto::{Hash, Signature}, encoding, helpers::Height, messages::RawTransaction,
    storage::{Fork, Snapshot},
};

const SERVICE_ID: u16 = 512;

transactions! {
    HandleCommitTransactions {
        const SERVICE_ID = SERVICE_ID;

        struct TxAfterCommit {
            height: Height,
        }
    }
}

impl Transaction for TxAfterCommit {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, _fork: &mut Fork) -> ExecutionResult {
        Ok(())
    }
}

pub struct AfterCommitService;

impl Service for AfterCommitService {
    fn service_name(&self) -> &str {
        "after_commit"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let tx = HandleCommitTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    fn after_commit(&self, context: &ServiceContext) {
        let tx = TxAfterCommit::new_with_signature(context.height(), &Signature::zero());
        context.transaction_sender().send(Box::new(tx)).unwrap();
    }
}
