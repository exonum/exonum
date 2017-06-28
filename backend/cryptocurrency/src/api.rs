//! Cryptocurrency API.

use serde::Serialize;
use serde_json::to_value;
use router::Router;
use iron::prelude::*;
use bodyparser;
use params::{Params, Value};

use std::fmt;

use exonum::api::{Api, ApiError};
use exonum::node::TransactionSend;
use exonum::messages::{BlockProof, Message};
use exonum::crypto::{HexValue, PublicKey, Hash};
use exonum::storage::{MapProof, ListProof};
use exonum::blockchain::{self, Blockchain};

use super::tx_metarecord::TxMetaRecord;
use super::wallet::Wallet;
use super::{CRYPTOCURRENCY_SERVICE_ID, CurrencySchema, CurrencyTx};

/// TODO: Add documentation.
#[derive(Debug, Serialize)]
pub struct MapProofTemplate<V: Serialize> {
    mpt_proof: MapProof<Hash>,
    value: V,
}

/// TODO: Add documentation.
#[derive(Debug, Serialize)]
pub struct ListProofTemplate<V: Serialize> {
    mt_proof: ListProof<TxMetaRecord>,
    values: Vec<V>,
}

/// Wallet information.
#[derive(Debug, Serialize)]
pub struct WalletInfo {
    block_info: BlockProof,
    wallet: MapProofTemplate<MapProof<Wallet>>,
    wallet_history: Option<ListProofTemplate<CurrencyTx>>,
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
    where T: TransactionSend + Clone
{
    fn wallet_info(&self, pub_key: &PublicKey) -> Result<WalletInfo, ApiError> {
        let view = self.blockchain.snapshot();
        let general_schema = blockchain::Schema::new(&view);
        let mut view = self.blockchain.fork();
        let mut currency_schema = CurrencySchema::new(&mut view);

        let max_height = general_schema.block_hashes_by_height().len() - 1;
        let block_proof = general_schema.block_and_precommits(max_height).unwrap();
        let state_hash = *block_proof.block.state_hash(); //debug code

        let wallet_path: MapProofTemplate<MapProof<Wallet>>;
        let wallet_history: Option<ListProofTemplate<CurrencyTx>>;

        let to_wallets_table: MapProof<Hash> =
            general_schema
                .get_proof_to_service_table(CRYPTOCURRENCY_SERVICE_ID, 0);

        {
            let wallets_root_hash = currency_schema.wallets_proof().root_hash(); //debug code
            let check_result = to_wallets_table.verify_root_proof_consistency(
                &Blockchain::service_table_unique_key(CRYPTOCURRENCY_SERVICE_ID, 0),
                state_hash); //debug code
            debug_assert_eq!(wallets_root_hash, *check_result.unwrap().unwrap());
        }

        let to_specific_wallet: MapProof<Wallet> =
            currency_schema.wallets_proof().get_proof(pub_key);
        wallet_path = MapProofTemplate {
            mpt_proof: to_wallets_table,
            value: to_specific_wallet,
        };

        wallet_history = match currency_schema.wallet(pub_key) {
            Some(wallet) => {
                let history = currency_schema.wallet_history(pub_key);
                let history_len = history.len();
                debug_assert!(history_len >= 1);
                debug_assert_eq!(history_len, wallet.history_len());
                let tx_records: Vec<TxMetaRecord> = history.into_iter().collect();
                let transactions_table = general_schema.transactions();
                let mut txs: Vec<CurrencyTx> = Vec::with_capacity(tx_records.len());
                for record in tx_records {
                    let raw_message = transactions_table.get(record.tx_hash()).unwrap();
                    txs.push(CurrencyTx::from(raw_message));
                }
                let to_transaction_hashes: ListProof<TxMetaRecord> =
                    history.get_range_proof(0, history_len);
                let path_to_transactions = ListProofTemplate {
                    mt_proof: to_transaction_hashes,
                    values: txs,
                };
                Some(path_to_transactions)
            }
            None => None,
        };
        let res = WalletInfo {
            block_info: block_proof,
            wallet: wallet_path,
            wallet_history: wallet_history,
        };
        Ok(res)
    }

    fn transaction(&self, tx: CurrencyTx) -> Result<Hash, ApiError> {
        let tx_hash = tx.hash();
        match self.channel.send(tx) {
            Ok(_) => Ok(tx_hash),
            Err(e) => Err(ApiError::Events(e)),
        }
    }
}

impl<T: Clone + TransactionSend> fmt::Debug for CryptocurrencyApi<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CryptocurrencyApi {{}}")
    }
}

impl<T> Api for CryptocurrencyApi<T>
    where T: 'static + TransactionSend + Clone
{
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let wallet_info = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            match map.find(&["pubkey"]) {
                Some(&Value::String(ref pub_key_string)) => {
                    let public_key = PublicKey::from_hex(pub_key_string)
                        .map_err(ApiError::FromHex)?;
                    let info = self_.wallet_info(&public_key)?;
                    self_.ok_response(&to_value(&info).unwrap())
                }
                _ => {
                    Err(ApiError::IncorrectRequest("Required parameter of \
                                                     wallet 'pubkey' is missing"
                                                           .into()))?
                }
            }
        };

        let self_ = self.clone();
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<CurrencyTx>>() {
                Ok(Some(transaction)) => {
                    let tx_hash = self_.transaction(transaction)?;
                    let json = TxResponse { tx_hash: tx_hash };
                    self_.ok_response(&to_value(&json).unwrap())
                }
                Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
            }
        };
        let route_post = "/v1/wallets/transaction";
        let route_get = "/v1/wallets/info";
        router.post(&route_post, transaction, "transaction");
        info!("Created post route: {}", route_post);
        router.get(&route_get, wallet_info, "wallet_info");
        info!("Created get route: {}", route_get);
    }
}

#[derive(Serialize, Deserialize)]
struct TxResponse {
    tx_hash: Hash,
}
