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

use exonum::{
    crypto,
    messages::{AnyTx, Verified},
    proto::Any,
    runtime::{rust::Transaction, ExecutionError},
};
use exonum_testkit::{InstanceCollection, TestKit, TestKitBuilder};

use schema::WalletSchema;
use services::{DepositService, TxCreateWallet, TxIssue, WalletService};

mod error;
mod interface;
mod proto;
mod schema;
mod services;

fn testkit_with_interfaces() -> TestKit {
    TestKitBuilder::validator()
        .with_logger()
        .with_service(InstanceCollection::new(WalletService).with_instance(
            WalletService::ID,
            "wallet",
            Any::default(),
        ))
        .with_service(InstanceCollection::new(DepositService).with_instance(
            DepositService::ID,
            "deposit",
            Any::default(),
        ))
        .create()
}

fn execute_transaction(testkit: &mut TestKit, tx: Verified<AnyTx>) -> Result<(), ExecutionError> {
    testkit.create_block_with_transaction(tx).transactions[0]
        .status()
        .map_err(Clone::clone)
}

#[test]
fn test_create_wallet() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        TxCreateWallet {
            name: "Alice".into(),
        }
        .sign(WalletService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to create wallet");
}

#[test]
fn test_deposit() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        TxCreateWallet {
            name: "Alice".into(),
        }
        .sign(WalletService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to create wallet");

    execute_transaction(
        &mut testkit,
        TxIssue {
            to: keypair.0,
            amount: 10_000,
        }
        .sign(DepositService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to deposit wallet");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletSchema::new(&snapshot)
            .wallets()
            .get(&keypair.0)
            .unwrap()
            .balance,
        10_000
    );
}
