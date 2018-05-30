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

//! This is a basic balance service. The only available operation is a wallet balance update.
//! Regression test for transactions processing is built on a base of this service.
extern crate bodyparser;
#[macro_use]
extern crate exonum;
extern crate futures;
extern crate iron;
extern crate router;
extern crate serde;
extern crate serde_json;

pub mod schema {
    use exonum::storage::{Entry, Fork, Snapshot};

    pub struct BalanceSchema<T> {
        view: T,
    }

    impl<T: AsRef<Snapshot>> BalanceSchema<T> {
        pub fn new(view: T) -> Self {
            Self { view }
        }

        pub fn balance(&self) -> Entry<&Snapshot, u64> {
            Entry::new("balance", self.view.as_ref())
        }
    }

    impl<'a> BalanceSchema<&'a mut Fork> {
        pub fn balance_mut(&mut self) -> Entry<&mut Fork, u64> {
            Entry::new("balance", self.view)
        }
    }
}

pub mod transactions {
    use service::SERVICE_ID;

    transactions! {
        pub BalanceTransactions {
            const SERVICE_ID = SERVICE_ID;

            struct TxAddBalance {
                amount: u64,
                seed: u64
            }
        }
    }
}

pub mod contracts {
    use exonum::blockchain::{ExecutionResult, Transaction};
    use exonum::storage::Fork;

    use schema::BalanceSchema;
    use transactions::TxAddBalance;

    impl Transaction for TxAddBalance {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, view: &mut Fork) -> ExecutionResult {
            let mut schema = BalanceSchema::new(view);
            let new_balance = schema.balance().get().unwrap() + self.amount();
            schema.balance_mut().set(new_balance);

            Ok(())
        }
    }
}

pub mod service {
    use exonum::blockchain::{Service, Transaction, TransactionSet};
    use exonum::crypto::{gen_keypair, Hash};
    use exonum::helpers;
    use exonum::node::{ExternalMessage, Node, TransactionSend};
    use exonum::storage::{Database, Fork, MemoryDB, Snapshot};
    use exonum::{encoding, messages::RawTransaction};
    use serde_json::Value;

    use schema::BalanceSchema;
    use transactions::{BalanceTransactions, TxAddBalance};

    use std::sync::Arc;
    use std::thread;
    use std::time;

    pub const SERVICE_ID: u16 = 1;

    pub struct BalanceService();

    impl Service for BalanceService {
        fn service_id(&self) -> u16 {
            SERVICE_ID
        }

        fn service_name(&self) -> &str {
            "balance"
        }

        fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
            vec![]
        }

        fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
            let tx = BalanceTransactions::tx_from_raw(raw)?;
            Ok(tx.into())
        }

        fn initialize(&self, fork: &mut Fork) -> Value {
            let mut schema = BalanceSchema::new(fork);
            schema.balance_mut().set(0);
            Value::Null
        }
    }

    #[test]
    fn test_duplicated_transaction() {
        let (_, private_key) = gen_keypair();

        let db = Arc::from(Box::new(MemoryDB::new()) as Box<Database>) as Arc<Database>;
        let mut node_cfg = helpers::generate_testnet_config(1, 16_500)[0].clone();

        // Override timeouts to little values, so we won't have to wait for consensus too long.
        node_cfg.genesis.consensus.min_propose_timeout = 0;
        node_cfg.genesis.consensus.max_propose_timeout = 0;
        node_cfg.genesis.consensus.propose_timeout_threshold = 0;
        node_cfg.genesis.consensus.round_timeout = 40;

        let service = Box::new(BalanceService());
        let node = Node::new(db.clone(), vec![service], node_cfg.clone());
        let api_tx = node.channel();

        let node_thread = thread::spawn(move || {
            node.run().unwrap();
        });

        let tx_orig = Box::new(TxAddBalance::new(10, 0, &private_key));
        let tx_copy = tx_orig.clone();

        // Send two identical transactions.
        api_tx.send(tx_orig).unwrap();

        api_tx.send(tx_copy).unwrap();

        // Wait to be sure that transaction was processed.
        thread::sleep(time::Duration::from_millis(200));

        // Shut down the node
        api_tx
            .send_external_message(ExternalMessage::Shutdown)
            .unwrap();

        node_thread.join().unwrap();

        // Check that only one transaction was accepted.
        let schema = BalanceSchema::new(db.snapshot());
        let balance = schema.balance().get().unwrap();

        assert_eq!(balance, 10);
    }
}
