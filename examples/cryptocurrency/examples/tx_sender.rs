use exonum_cryptocurrency::transactions::{TxCreateWallet, TxTransfer};

use std::thread::sleep;

use exonum::{
    crypto::{gen_keypair, PublicKey, SecretKey},
    messages::to_hex_string,
    messages::{AnyTx, BinaryForm, CallInfo, Message},
};
use serde_json::json;

fn encode_signed_tx<T: BinaryForm>(
    tx: T,
    method_id: u32,
    pk: &PublicKey,
    sk: &SecretKey,
) -> serde_json::Value {
    let any_tx = AnyTx {
        dispatch: CallInfo {
            instance_id: 1,
            method_id,
        },
        payload: tx.encode().unwrap(),
    };
    let hex = to_hex_string(&Message::concrete(any_tx, *pk, sk));
    json!({ "tx_body": hex })
}

fn send_tx(tx: serde_json::Value, client: &reqwest::Client) {
    let endpoint = "http://127.0.0.1:8000/api/explorer/v1/transactions";
    client
        .post(endpoint)
        .json(&tx)
        .send()
        .map(|res| println!("res status: {}", res.status()))
        .map_err(|e| eprintln!("Failed to send request: {}", e))
        .unwrap_or_default();
}

fn main() {
    let client = reqwest::Client::new();

    let (apk, ask) = gen_keypair();
    let a_wallet = encode_signed_tx(
        TxCreateWallet {
            name: "alice".to_owned(),
        },
        0,
        &apk,
        &ask,
    );
    send_tx(a_wallet, &client);

    sleep(std::time::Duration::from_secs(1));

    let (bpk, bsk) = gen_keypair();
    let b_wallet = encode_signed_tx(
        TxCreateWallet {
            name: "bob".to_owned(),
        },
        0,
        &bpk,
        &bsk,
    );
    send_tx(b_wallet, &client);
    sleep(std::time::Duration::from_secs(1));

    let a_to_b = encode_signed_tx(
        TxTransfer {
            to: bpk,
            amount: 14,
            seed: 0,
        },
        1,
        &apk,
        &ask,
    );
    send_tx(a_to_b, &client);
}
