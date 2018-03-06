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

#[macro_use]
extern crate exonum;
#[macro_use]
extern crate exonum_derive;
extern crate serde_json;

use exonum::blockchain::{ExecutionResult, Transaction, TransactionSet};
use exonum::crypto::{self, PublicKey};
use exonum::messages::Message;
use exonum::storage::Fork;

messages! {
    const SERVICE_ID = 1000;

    struct CreateWallet {
        public_key: &PublicKey,
        name: &str,
    }

    struct Transfer {
        from: &PublicKey,
        to: &PublicKey,
        amount: u64,
    }
}

impl Transaction for CreateWallet {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, _: &mut Fork) -> ExecutionResult {
        Ok(())
    }
}

impl Transaction for Transfer {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, _: &mut Fork) -> ExecutionResult {
        Ok(())
    }
}

mod sub {
    pub use super::Transfer;
}

#[derive(Clone, TransactionSet)]
pub enum Transactions {
    CreateWallet(CreateWallet),
    Transfer(sub::Transfer),
}

#[test]
fn test_transaction_set() {
    let (pubkey, key) = crypto::gen_keypair();
    let tx = CreateWallet::new(&pubkey, "Alice", &key);
    let raw = tx.raw().clone();
    let json = serde_json::to_value(&tx).unwrap();

    {
        let generic_tx = Transactions::CreateWallet(tx);
        let generic_json = serde_json::to_value(&generic_tx).unwrap();
        assert_eq!(generic_json, json);
    }

    let parsed_tx: Transactions = serde_json::from_value(json).unwrap();
    match parsed_tx {
        Transactions::CreateWallet(..) => {}
        _ => panic!("Unexpected transaction type"),
    }

    let parsed_tx = Transactions::tx_from_raw(raw.clone()).unwrap();
    match parsed_tx {
        Transactions::CreateWallet(..) => {}
        _ => panic!("Unexpected transaction type"),
    }

    let boxed_tx: Box<Transaction> = parsed_tx.into();
    assert_eq!(*boxed_tx.raw(), raw);

    let json = serde_json::to_value(&Transfer::new(&pubkey, &pubkey, 10, &key)).unwrap();
    let parsed_tx: Transactions = serde_json::from_value(json).unwrap();
    match parsed_tx {
        Transactions::Transfer(..) => {}
        _ => panic!("Unexpected transaction type"),
    }
}
