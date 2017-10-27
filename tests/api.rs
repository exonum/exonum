extern crate cryptocurrency;
extern crate exonum;
extern crate exonum_harness;
extern crate iron;

use exonum::crypto::{self, PublicKey, SecretKey};
use exonum::messages::Message;
use exonum_harness::{TestHarness, HarnessApi};

use cryptocurrency::{TxCreateWallet, TxTransfer, TransactionResponse, Wallet, blockchain};

fn create_wallet(api: &HarnessApi, name: &str) -> (TxCreateWallet, SecretKey) {
    let (pubkey, key) = crypto::gen_keypair();
    // Create a presigned transaction
    let tx = TxCreateWallet::new(&pubkey, name, &key);

    let tx_info: TransactionResponse = api.post("cryptocurrency", "v1/wallets/transaction", &tx);
    assert_eq!(tx_info.tx_hash, tx.hash());

    (tx, key)
}

fn get_wallet(api: &HarnessApi, pubkey: &PublicKey) -> Wallet {
    api.get(
        "cryptocurrency",
        &format!("v1/wallet/{}", pubkey.to_string()),
    )
}

#[test]
fn test_create_wallet() {
    let mut harness = TestHarness::new(blockchain());
    let api = harness.api();
    let (tx, _) = create_wallet(&api, "Alice");

    harness.create_block();

    // Check that the user indeed is persisted by the service
    let wallet = get_wallet(&api, tx.pub_key());
    assert_eq!(wallet.pub_key(), tx.pub_key());
    assert_eq!(wallet.name(), tx.name());
    assert_eq!(wallet.balance(), 100);
}

#[test]
fn test_transfer() {
    let mut harness = TestHarness::new(blockchain());
    let api = harness.api();
    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");

    harness.create_block();

    // Check that the initial Alice's and Bob's balances persisted by the service
    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 100);
    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);

    // Transfer funds
    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        10, // amount
        0, // seed
        &key_alice,
    );
    let tx_info: TransactionResponse = api.post("cryptocurrency", "v1/wallets/transaction", &tx);
    assert_eq!(tx_info.tx_hash, tx.hash());

    harness.create_block();

    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 90);
    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 110);
}
