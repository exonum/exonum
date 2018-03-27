//! Cryptocurrency API.

use serde::Serialize;
use serde_json;
use router::Router;
use iron::prelude::*;
use bodyparser;

use exonum::api::{Api, ApiError};
use exonum::node::TransactionSend;
use exonum::crypto::{PublicKey, Hash};
use exonum::storage::{MapProof, ListProof};
use exonum::blockchain::{self, Blockchain, BlockProof, Transaction, TransactionSet};
use exonum::helpers::Height;

use std::fmt;

use {CRYPTOCURRENCY_SERVICE_ID, CurrencySchema};
use schema::MetaRecord;
use transactions::WalletTransactions;
use wallet::Wallet;

/// The structure returned by the REST API.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResponse {
    /// Hash of the transaction.
    pub tx_hash: Hash,
}

/// Proof of existence for specific wallet.
#[derive(Debug, Serialize)]
pub struct WalletProof {
    /// Proof to the whole database table.
    to_table: MapProof<Hash>,
    /// Proof to the specific wallet in this table.
    to_wallet: MapProof<Wallet>,
}

/// Proof to wallet history.
#[derive(Debug, Serialize)]
pub struct WalletHistoryProof {
    proof: ListProof<MetaRecord>,
    transactions: Vec<WalletTransactions>,
}

/// Wallet information.
#[derive(Debug, Serialize)]
pub struct WalletInfo {
    block_proof: BlockProof,
    wallet: Option<Wallet>,
    wallet_proof: WalletProof,
    wallet_history: Option<WalletHistoryProof>,
}

/// TODO: Add documentation.
#[derive(Clone)]
pub struct CryptocurrencyApi<T: TransactionSend + Clone> {
    /// Exonum blockchain.
    pub blockchain: Blockchain,
    /// Channel for transactions.
    pub channel: T,
}

impl<T> CryptocurrencyApi<T>
where
    T: TransactionSend + Clone + 'static,
{
    fn wallet_info(&self, pub_key: &PublicKey) -> Result<WalletInfo, ApiError> {
        let view = self.blockchain.snapshot();
        let general_schema = blockchain::Schema::new(&view);
        let mut view = self.blockchain.fork();
        let mut currency_schema = CurrencySchema::new(&mut view);

        let max_height = general_schema.block_hashes_by_height().len() - 1;

        let block_proof = general_schema
            .block_and_precommits(Height(max_height))
            .unwrap();

        let to_table: MapProof<Hash> =
            general_schema.get_proof_to_service_table(CRYPTOCURRENCY_SERVICE_ID, 0);

        let to_wallet: MapProof<Wallet> = currency_schema.wallets().get_proof(pub_key);

        let wallet_proof = WalletProof {
            to_table,
            to_wallet,
        };

        let wallet = currency_schema.wallet(pub_key);

        let wallet_history = if wallet.is_some() {
            let history = currency_schema.wallet_history(pub_key);

            let proof: ListProof<MetaRecord> = history.get_range_proof(0, history.len());

            let transactions: Vec<WalletTransactions> = history
                .iter()
                .map(|record| {
                    general_schema
                        .transactions()
                        .get(record.transaction_hash())
                        .unwrap()
                })
                .map(|raw| WalletTransactions::tx_from_raw(raw).unwrap())
                .collect::<Vec<_>>();

            Some(WalletHistoryProof {
                proof,
                transactions,
            })
        } else {
            None
        };

        Ok(WalletInfo {
            block_proof,
            wallet,
            wallet_proof,
            wallet_history,
        })
    }

    fn post_transaction(&self, req: &mut Request) -> IronResult<Response> {
        match req.get::<bodyparser::Struct<WalletTransactions>>() {
            Ok(Some(transaction)) => {
                let transaction: Box<Transaction> = transaction.into();
                let tx_hash = transaction.hash();
                self.channel.send(transaction).map_err(ApiError::from)?;
                let json = TransactionResponse { tx_hash };
                self.ok_response(&serde_json::to_value(&json).unwrap())
            }
            Ok(None) => Err(ApiError::BadRequest("Empty request body".into()))?,
            Err(e) => Err(ApiError::BadRequest(e.to_string()))?,
        }
    }

    fn wire_post_transaction(self, router: &mut Router) {
        let transaction = move |req: &mut Request| self.post_transaction(req);
        router.post("/v1/wallets/transaction", transaction, "post_transaction");
    }

    fn wire_wallet_info(self, router: &mut Router) {
        let wallet_info = move |req: &mut Request| -> IronResult<Response> {
            let pub_key: PublicKey = self.url_fragment(req, "pubkey")?;
            let info = self.wallet_info(&pub_key)?;
            self.ok_response(&serde_json::to_value(&info).unwrap())
        };
        router.get("/v1/wallets/info/:pubkey", wallet_info, "wallet_info");
    }
}

impl<T: TransactionSend + Clone> fmt::Debug for CryptocurrencyApi<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CryptocurrencyApi {{}}")
    }
}

impl<T> Api for CryptocurrencyApi<T>
where
    T: 'static + TransactionSend + Clone,
{
    fn wire(&self, router: &mut Router) {
        self.clone().wire_post_transaction(router);
        self.clone().wire_wallet_info(router);
    }
}
