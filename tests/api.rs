extern crate cryptocurrency;
extern crate exonum;
#[macro_use]
extern crate exonum_testkit;

use exonum::crypto::{self, PublicKey, SecretKey};
use exonum::messages::Message;
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};

use cryptocurrency::{TxCreateWallet, TxTransfer, TransactionResponse, Wallet, CurrencyService};

struct CryptocurrencyApi {
    inner: TestKitApi,
}

impl CryptocurrencyApi {
    fn create_wallet(&self, name: &str) -> (TxCreateWallet, SecretKey) {
        let (pubkey, key) = crypto::gen_keypair();
        // Create a presigned transaction
        let tx = TxCreateWallet::new(&pubkey, name, &key);

        let tx_info: TransactionResponse = self.inner.post(
            ApiKind::Service("cryptocurrency"),
            "v1/wallets",
            &tx,
        );
        assert_eq!(tx_info.tx_hash, tx.hash());
        (tx, key)
    }

    fn get_wallet(&self, pubkey: &PublicKey) -> Wallet {
        self.inner.get(
            ApiKind::Service("cryptocurrency"),
            &format!("v1/wallet/{}", pubkey.to_string()),
        )
    }
}

fn create_testkit() -> (TestKit, CryptocurrencyApi) {
    let testkit = TestKitBuilder::validator()
        .with_service(CurrencyService)
        .create();
    let api = CryptocurrencyApi { inner: testkit.api() };
    (testkit, api)
}

#[test]
fn test_create_wallet() {
    let (mut testkit, api) = create_testkit();
    let (tx, _) = api.create_wallet("Alice");
    testkit.create_block();

    // Check that the user indeed is persisted by the service
    let wallet = api.get_wallet(tx.pub_key());
    assert_eq!(wallet.pub_key(), tx.pub_key());
    assert_eq!(wallet.name(), tx.name());
    assert_eq!(wallet.balance(), 100);
}

#[test]
fn test_transfer() {
    let (mut testkit, api) = create_testkit();
    let (tx_alice, key_alice) = api.create_wallet("Alice");
    let (tx_bob, _) = api.create_wallet("Bob");
    testkit.create_block();

    // Check that the initial Alice's and Bob's balances persisted by the service
    let wallet = api.get_wallet(tx_alice.pub_key());
    assert_eq!(wallet.balance(), 100);
    let wallet = api.get_wallet(tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);

    // Transfer funds
    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        10, // amount
        0, // seed
        &key_alice,
    );
    testkit.create_block_with_transactions(txvec![tx]);

    let wallet = api.get_wallet(tx_alice.pub_key());
    assert_eq!(wallet.balance(), 90);
    let wallet = api.get_wallet(tx_bob.pub_key());
    assert_eq!(wallet.balance(), 110);
}
