use rand::thread_rng;
use rand::Rng;
use serde_json::value::ToJson;
use serde_json::Value;

use router::Router;
use iron::prelude::*;
use bodyparser;
use jsonway;

use blockchain_explorer::BlockchainExplorer;
use blockchain_explorer::api::{Api, ApiError};

use exonum::crypto::{gen_keypair, SecretKey, Hash};
use exonum::storage::StorageValue;

use exonum::crypto::{HexValue, PublicKey};
use exonum::blockchain::Blockchain;

use super::wallet::{Wallet, WalletId};
use super::CurrencyTxSender;
use super::{CurrencySchema, CurrencyTx, TxIssue, TxTransfer, TxCreateWallet};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WalletRequest {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IssueRequest {
    pub amount: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferRequest {
    pub amount: i64,
    pub to: String,
}

#[derive(Serialize)]
pub struct WalletInfo {
    inner: Wallet,
    id: WalletId,
    history: Vec<Value>,
}

#[derive(Clone)]
pub struct CryptocurrencyApi {
    pub blockchain: Blockchain,
    pub channel: CurrencyTxSender,
}

impl CryptocurrencyApi {
    fn wallet_info(&self, pub_key: &PublicKey) -> Result<WalletInfo, ApiError> {
        let view = self.blockchain.view();
        let schema = CurrencySchema::new(&view);
        match schema.wallet(&pub_key)? {
            Some((id, wallet)) => {
                let history = schema.wallet_history(id).values()?;
                let txs = {
                    let mut v = Vec::new();
                    let explorer = BlockchainExplorer::new(&self.blockchain);
                    for hash in history {
                        if let Some(tx_info) = explorer.tx_info(&hash)? {
                            v.push(tx_info)
                        }
                    }
                    v
                };
                let info = WalletInfo {
                    id: id,
                    inner: wallet,
                    history: txs,
                };
                Ok(info)
            }
            None => Err(ApiError::IncorrectRequest),
        }

    }

    fn wallet_issue(&self,
                    issue: IssueRequest,
                    public_key: &PublicKey,
                    secret_key: &SecretKey)
                    -> Result<Hash, ApiError> {

        let seed = thread_rng().gen::<u64>();
        let tx = TxIssue::new(public_key, issue.amount, seed, secret_key);

        let tx_hash = tx.hash();
        let ch = self.channel.clone();
        match ch.send(CurrencyTx::from(tx)) {
            Ok(_) => Ok(tx_hash),
            Err(e) => Err(ApiError::Events(e)),
        }

    }

    fn wallet_xfer(&self,
                   transfer: TransferRequest,
                   public_key: &PublicKey,
                   secret_key: &SecretKey)
                   -> Result<Hash, ApiError> {

        let seed = thread_rng().gen::<u64>();

        match PublicKey::from_hex(transfer.to) {
            Ok(to_key) => {
                let tx = TxTransfer::new(public_key, &to_key, transfer.amount, seed, secret_key);
                let tx_hash = tx.hash();
                let ch = self.channel.clone();
                match ch.send(CurrencyTx::from(tx)) {
                    Ok(_) => Ok(tx_hash),
                    Err(e) => Err(ApiError::Events(e)),
                }
            }
            Err(e) => Err(ApiError::FromHex(e)),
        }
    }

    pub fn wallet_create(&self,
                         wallet: WalletRequest,
                         public_key: &PublicKey,
                         secret_key: &SecretKey)
                         -> Result<Hash, ApiError> {

        let tx = TxCreateWallet::new(public_key, &wallet.name, secret_key);
        let tx_hash = tx.hash();
        let ch = self.channel.clone();
        match ch.send(CurrencyTx::from(tx)) {
            Ok(_) => Ok(tx_hash),
            Err(e) => Err(ApiError::Events(e)),
        }
    }
}

impl Api for CryptocurrencyApi {
    fn wire(&self, router: &mut Router) {

        let _self = self.clone();
        let wallet_info = move |req: &mut Request| -> IronResult<Response> {
            let (public_key, _) = _self.load_keypair_from_cookies(&req)?;
            let info = _self.wallet_info(&public_key)?;
            _self.ok_response(&info.to_json())
        };

        let _self = self.clone();
        let wallet_issue = move |req: &mut Request| -> IronResult<Response> {
            let (public_key, secret_key) = _self.load_keypair_from_cookies(&req)?;
            match req.get::<bodyparser::Struct<IssueRequest>>().unwrap() {
                Some(issue) => {
                    let tx_hash = _self.wallet_issue(issue, &public_key, &secret_key)?;
                    let json = &jsonway::object(|json| json.set("tx_hash", tx_hash)).unwrap();
                    _self.ok_response(json)
                }
                None => Err(ApiError::IncorrectRequest)?,
            }
        };

        let _self = self.clone();
        let wallet_xfer = move |req: &mut Request| -> IronResult<Response> {
            let (public_key, secret_key) = _self.load_keypair_from_cookies(&req)?;
            match req.get::<bodyparser::Struct<TransferRequest>>().unwrap() {
                Some(transfer) => {
                    let tx_hash = _self.wallet_xfer(transfer, &public_key, &secret_key)?;
                    let json = &jsonway::object(|json| json.set("tx_hash", tx_hash)).unwrap();
                    _self.ok_response(json)
                }
                None => Err(ApiError::IncorrectRequest)?,
            }
        };

        let _self = self.clone();
        let wallet_create = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<WalletRequest>>().unwrap() {
                Some(wallet) => {
                    let (public_key, secret_key) = gen_keypair();
                    let tx_hash = _self.wallet_create(wallet, &public_key, &secret_key)?;
                    let json = &jsonway::object(|json| json.set("tx_hash", tx_hash)).unwrap();
                    _self.ok_response_with_cookies(json,
                                                   Some(vec![format!("public_key={}",
                                                                     public_key.to_hex()),
                                                             format!("secret_key={}",
                                                                     secret_key.to_hex())]))
                }
                None => Err(ApiError::IncorrectRequest)?,
            }
        };

        router.post("/v1/api/wallets/create", wallet_create, "wallets_create");
        router.post("/v1/api/wallets/issue", wallet_issue, "wallets_issue");
        router.post("/v1/api/wallets/transfer", wallet_xfer, "wallets_transfer");
        router.get("/v1/api/wallets/info", wallet_info, "wallets_info");
    }
}
