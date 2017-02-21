use serde::Serialize;
use serde_json::value::ToJson;

use router::Router;
use iron::prelude::*;
use bodyparser;
use jsonway;
use params::{Params, Value};
use blockchain_explorer::api::{Api, ApiError};

use exonum::messages::BlockProof;
use exonum::crypto::{HexValue, PublicKey, Hash};
use exonum::storage::{StorageValue, List, Map};
/// Merkle tree Proof
use exonum::storage::Proofnode;
/// Merkle Patricia tree Proof
use exonum::storage::RootProofNode;

use exonum::blockchain::{self, Blockchain};

use super::wallet::Wallet;
use super::CurrencyTxSender;
use super::{CRYPTOCURRENCY, CurrencySchema, CurrencyTx};

#[derive(Serialize)]
pub struct HashMPTproofLinker<V: Serialize> {
    mpt_proof: RootProofNode<Hash>,
    value: V,
}

#[derive(Serialize)]
pub struct HashMTproofLinker<V: Serialize> {
    mt_proof: Proofnode<Hash>,
    values: Vec<V>,
}

#[derive(Serialize)]
pub struct WalletInfo {
    block_info: BlockProof,
    wallet: HashMPTproofLinker<RootProofNode<Wallet>>,
    wallet_history: Option<HashMTproofLinker<CurrencyTx>>,
}

#[derive(Clone)]
pub struct CryptocurrencyApi {
    pub blockchain: Blockchain,
    pub channel: CurrencyTxSender,
}

impl CryptocurrencyApi {
    fn wallet_info(&self, pub_key: &PublicKey) -> Result<WalletInfo, ApiError> {
        let view = self.blockchain.view();
        let general_schema = blockchain::Schema::new(&view);
        let currency_schema = CurrencySchema::new(&view);

        let max_height = general_schema.heights().len()? - 1;
        let block_proof = general_schema.block_and_precommits(max_height)?.unwrap();
        let state_hash = *block_proof.block.state_hash(); //debug code

        let wallet_path: HashMPTproofLinker<RootProofNode<Wallet>>;
        let wallet_history: Option<HashMTproofLinker<CurrencyTx>>;

        let to_wallets_table: RootProofNode<Hash> =
            general_schema.get_proof_to_service_table(CRYPTOCURRENCY, 0)?;

        {
            let wallets_root_hash = currency_schema.wallets().root_hash()?; //debug code
            let check_result = to_wallets_table.verify_root_proof_consistency(Blockchain::service_table_unique_key(CRYPTOCURRENCY, 0), state_hash); //debug code
            debug_assert_eq!(wallets_root_hash, *check_result.unwrap().unwrap());
        }

        let to_specific_wallet: RootProofNode<Wallet> = currency_schema.wallets()
            .construct_path_to_key(pub_key)?;
        wallet_path = HashMPTproofLinker {
            mpt_proof: to_wallets_table,
            value: to_specific_wallet,
        };

        wallet_history = match currency_schema.wallet(pub_key)? {
            Some(wallet) => {
                let history = currency_schema.wallet_history(pub_key);
                let history_len = history.len()?;
                debug_assert!(history_len >= 1);
                debug_assert_eq!(history_len, wallet.history_len());
                let tx_hashes: Vec<Hash> = history.values()?;
                let transactions_table = general_schema.transactions();
                let mut txs: Vec<CurrencyTx> = Vec::with_capacity(tx_hashes.len());
                for hash in tx_hashes {
                    let raw_message = transactions_table.get(&hash)?.unwrap();
                    txs.push(CurrencyTx::from(raw_message));
                }
                let to_transaction_hashes: Proofnode<Hash> =
                    history.construct_path_for_range(0, history_len)?;
                let path_to_transactions = HashMTproofLinker {
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
        let ch = self.channel.clone();
        match ch.send(tx) {
            Ok(_) => Ok(tx_hash),
            Err(e) => Err(ApiError::Events(e)),
        }
    }
}

impl Api for CryptocurrencyApi {
    fn wire(&self, router: &mut Router) {
        let _self = self.clone();
        let wallet_info = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            match map.find(&["pub_key"]) {
                Some(&Value::String(ref pub_key_string)) => {
                    let public_key =
                        PublicKey::from_hex(pub_key_string).map_err(|err| ApiError::FromHex(err))?;
                    let info = _self.wallet_info(&public_key)?;
                    _self.ok_response(&info.to_json())
                }
                _ => return Err(ApiError::IncorrectRequest)?,
            }
        };

        let _self = self.clone();
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<CurrencyTx>>().unwrap() {
                Some(transaction) => {
                    let tx_hash = _self.transaction(transaction)?;
                    let json = &jsonway::object(|json| json.set("tx_hash", tx_hash)).unwrap();
                    _self.ok_response(json)
                }
                None => Err(ApiError::IncorrectRequest)?,
            }
        };

        router.post("/v1/api/wallets/transaction", transaction, "transaction");
        router.get("/v1/api/wallets/info/:pub_key", wallet_info, "wallet_info");
    }
}
