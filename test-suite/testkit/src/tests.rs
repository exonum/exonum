use super::*;
use crate::{TestKit, TestKitApi, TestKitBuilder};
use exonum::{
    crypto::gen_keypair,
    explorer::CommittedTransaction,
    messages::{AnyTx, Verified},
    runtime::rust::Transaction,
};
use exonum_cryptocurrency::contracts::CryptocurrencyService;
use exonum_cryptocurrency::transactions::CreateWallet;

const SERVICE_ID: u32 = 2;
const SERVICE_NAME: &str = "cryptocurrency";

fn init_testkit() -> (TestKit, TestKitApi) {
    let service = CryptocurrencyService;
    let artifact = service.artifact_id();
    let mut testkit = TestKitBuilder::validator()
        .with_artifact(artifact.clone())
        .with_instance(artifact.into_default_instance(SERVICE_ID, SERVICE_NAME))
        .with_rust_service(service)
        .create();
    let api = testkit.api();
    (testkit, api)
}

fn gen_tx(name: String) -> Verified<AnyTx> {
    let (pubkey, key) = gen_keypair();
    CreateWallet { name: name }.sign(SERVICE_ID, pubkey, &key)
}

fn gen_incorrect_tx(name: String) -> Verified<AnyTx> {
    let (pubkey, key) = gen_keypair();
    CreateWallet { name: name }.sign(SERVICE_ID + 1, pubkey, &key)
}

#[test]
fn testkit_add_tx() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_tx("tx".to_string());
    testkit.add_tx(tx.clone());
    assert!(testkit.is_tx_in_pool(&tx.object_hash()));
}

#[test]
#[should_panic(
    expected = "Attempt to add invalid tx in the pool: Execution error `dispatcher:7` \
                occurred: Suitable runtime for the given service instance ID is not found."
)]
fn teskit_add_tx_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_incorrect_tx("tx".to_string());
    testkit.add_tx(incorrect_tx);
}

#[test]
fn testkit_probe_all() {
    let (mut testkit, _) = init_testkit();
    let tx0 = gen_tx("tx0".to_string());
    let tx1 = gen_tx("tx1".to_string());
    let snapshot = testkit.probe_all(vec![tx0.clone(), tx1.clone()]);
    let schema = snapshot.for_core();
    assert!(
        schema.transactions().contains(&tx0.object_hash())
            && schema.transactions().contains(&tx1.object_hash())
    );
}

#[test]
#[should_panic(
    expected = "Attempt to add invalid tx in the pool: Execution error `dispatcher:7` \
                occurred: Suitable runtime for the given service instance ID is not found."
)]
fn testkit_probe_all_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_incorrect_tx("tx".to_string());
    testkit.probe_all(vec![incorrect_tx]);
}

#[test]
fn testkit_probe() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_tx("tx".to_string());
    let snapshot = testkit.probe(tx.clone());
    let schema = snapshot.for_core();
    assert!(schema.transactions().contains(&tx.object_hash()));
}

#[test]
#[should_panic(
    expected = "Attempt to add invalid tx in the pool: Execution error `dispatcher:7` \
                occurred: Suitable runtime for the given service instance ID is not found."
)]
fn testkit_probe_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_incorrect_tx("tx".to_string());
    testkit.probe(incorrect_tx);
}

#[test]
fn testkit_create_block_with_transactions() {
    let (mut testkit, _) = init_testkit();
    let tx0 = gen_tx("tx0".to_string());
    let tx1 = gen_tx("tx1".to_string());
    let block = testkit.create_block_with_transactions(vec![tx0.clone(), tx1.clone()]);

    assert!(
        block
            .iter()
            .map(CommittedTransaction::content)
            .any(|tx| *tx == tx0),
        "Transaction tx0 should be in the block."
    );
    assert!(
        block
            .iter()
            .map(CommittedTransaction::content)
            .any(|tx| *tx == tx1),
        "Transaction tx1 should be in the block."
    );

    let snapshot = testkit.snapshot();
    let txs = snapshot.for_core().transactions();
    assert!(
        txs.contains(&tx0.object_hash()) && txs.contains(&tx1.object_hash()),
        "Transactions should be in scheme."
    );
}

#[test]
#[should_panic(expected = "Transaction is already committed")]
fn testkit_create_block_with_transactions_commit_duplicated_transaction() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_tx("tx".to_string());
    testkit.create_block_with_transactions(vec![tx.clone()]);
    testkit.create_block_with_transactions(vec![tx.clone()]);
}

#[test]
#[should_panic(
    expected = "Attempt to add invalid tx in the pool: Execution error `dispatcher:7` \
                occurred: Suitable runtime for the given service instance ID is not found."
)]
fn testkit_create_block_with_transactions_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_incorrect_tx("tx".to_string());
    testkit.create_block_with_transactions(vec![incorrect_tx]);
}

#[test]
fn testkit_create_block_with_transaction() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_tx("tx".to_string());
    let block = testkit.create_block_with_transaction(tx.clone());
    assert!(
        block
            .iter()
            .map(CommittedTransaction::content)
            .any(|in_tx| *in_tx == tx),
        "Transaction tx should be in the block."
    );

    let snapshot = testkit.snapshot();
    let txs = snapshot.for_core().transactions();
    assert!(
        txs.contains(&tx.object_hash()),
        "Transaction should be in scheme."
    );
}

#[test]
#[should_panic(expected = "Transaction is already committed")]
fn testkit_create_block_with_transaction_commit_duplicated_transaction() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_tx("tx".to_string());
    testkit.create_block_with_transaction(tx.clone());
    testkit.create_block_with_transaction(tx.clone());
}

#[test]
#[should_panic(
    expected = "Attempt to add invalid tx in the pool: Execution error `dispatcher:7` \
                occurred: Suitable runtime for the given service instance ID is not found."
)]
fn testkit_create_block_with_transaction_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_incorrect_tx("tx".to_string());
    testkit.create_block_with_transaction(incorrect_tx);
}
