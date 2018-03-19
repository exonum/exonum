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

#[macro_use]
extern crate exonum;
#[macro_use]
extern crate exonum_derive;
#[macro_use]
extern crate serde_json;

use exonum::crypto::{self, PublicKey};
use exonum::messages::{Message, MessageWriter, RawMessage, Write};
use exonum::storage::StorageValue;

pub const SERVICE_ID: u16 = 1000;

encoding_struct! {
    struct Wallet {
        public_key: &PublicKey,
        name: &str,
        balance: u64,
    }
}

#[derive(Debug, MessageSet)]
#[exonum(service_id = "SERVICE_ID")]
pub enum Transactions<'r> {
    CreateWallet {
        creator: &'r PublicKey,
        wallet: Wallet,
    },

    CreateWallets {
        creator: &'r PublicKey,
        wallets: Vec<Wallet>,
    },

    Transfer {
        from: &'r PublicKey,
        to: &'r PublicKey,
        amount: u64,
        seed: u64,
    },
}

#[derive(Clone, Message)]
#[exonum(payload = "Transactions")]
pub struct MyMessage(RawMessage);

#[derive(Debug, MessageSet)]
#[exonum(service_id = "100")]
pub struct CreateWallet<'r> {
    pub creator: &'r PublicKey,
    pub wallet: Wallet,
}

impl<'r> Write<MyMessage> for Transactions<'r> {
    fn write_payload(&self, writer: &mut MessageWriter) {
        <Self as Write<RawMessage>>::write_payload(self, writer);
    }
}

#[test]
fn test_message_basics() {
    let (public_key, secret_key) = crypto::gen_keypair();

    let raw: RawMessage = Transactions::CreateWallet {
        creator: &public_key,
        wallet: Wallet::new(&public_key, "Alice", 200),
    }.sign(&secret_key);
    assert!(raw.verify_signature(&public_key));

    let message = MyMessage::from_raw(raw).unwrap();
    match message.payload() {
        Transactions::CreateWallet { ref wallet, .. } => {
            assert_eq!(wallet.name(), "Alice");
            assert_eq!(wallet.balance(), 200);
        }
        p => panic!("Unexpected payload: {:?}", p),
    }

    let json = serde_json::to_value(&message).unwrap();
    assert_eq!(
        json,
        json!({
            "protocol_version": 0,
            "network_id": 0,
            "service_id": 1000,
            "message_id": 0,
            "body": {
                "creator": public_key,
                "wallet": {
                    "public_key": public_key,
                    "name": "Alice",
                    "balance": "200",
                }
            },
            "signature": message.raw().signature(),
        })
    );

    let message_copy: MyMessage = serde_json::from_value(json).unwrap();
    assert_eq!(message.raw(), message_copy.raw());

    // `CreateWallets` variant

    let message: MyMessage = Transactions::CreateWallets {
        creator: &public_key,
        wallets: vec![
            Wallet::new(&public_key, "Alice", 200),
            Wallet::new(&public_key, "Bob", 5),
        ],
    }.sign(&secret_key);
    assert!(message.verify_signature(&public_key));

    match message.payload() {
        Transactions::CreateWallets { ref wallets, .. } => {
            assert_eq!(wallets[1].name(), "Bob");
            assert_eq!(wallets[1].balance(), 5);
        }
        p => panic!("Unexpected payload: {:?}", p),
    }

    let json = serde_json::to_value(&message).unwrap();
    assert_eq!(
        json,
        json!({
            "protocol_version": 0,
            "network_id": 0,
            "service_id": 1000,
            "message_id": 1,
            "body": {
                "creator": public_key,
                "wallets": [
                    {
                        "public_key": public_key,
                        "name": "Alice",
                        "balance": "200",
                    },
                    {
                        "public_key": public_key,
                        "name": "Bob",
                        "balance": "5",
                    },
                ]
            },
            "signature": message.raw().signature(),
        })
    );

    let message_copy: MyMessage = serde_json::from_value(json).unwrap();
    assert_eq!(message.raw(), message_copy.raw());
}


#[test]
fn test_message_as_storage_value() {
    encoding_struct! {
        struct MessageHolder {
            message: MyMessage,
            info: &str,
        }
    }

    let alice_keys = crypto::gen_keypair();
    let bob_keys = crypto::gen_keypair();

    let message: MyMessage = Transactions::Transfer {
        from: &alice_keys.0,
        to: &bob_keys.0,
        amount: 10,
        seed: 0,
    }.sign(&alice_keys.1);
    let raw = message.raw().clone();

    let holder = MessageHolder::new(message, "transfer");

    let bytes = holder.into_bytes();
    let holder = MessageHolder::from_bytes(bytes.into());
    assert_eq!(holder.info(), "transfer");
    assert_eq!(*holder.message().raw(), raw);
    match holder.message().payload() {
        Transactions::Transfer { amount, .. } => assert_eq!(amount, 10),
        p => panic!("Unexpected payload: {:?}", p),
    }
}

#[test]
fn test_derive_message_set_with_unnamed_fields() {
    #[derive(Debug, MessageSet)]
    #[exonum(service_id = "SERVICE_ID")]
    enum UnnamedTransactions<'r> {
        Create(&'r PublicKey, &'r str),
        Transfer(&'r PublicKey, &'r PublicKey, u64, u64),
    }

    #[derive(Message)]
    #[exonum(payload = "UnnamedTransactions")]
    struct UnnamedMessage(RawMessage);

    let (public_key, secret_key) = crypto::gen_keypair();

    let message = UnnamedMessage(UnnamedTransactions::Create(&public_key, "Carol").sign(
        &secret_key,
    ));
    assert_eq!(message.raw().message_type(), 0);

    let json = serde_json::to_value(&message).unwrap();
    assert_eq!(
        json,
        json!({
            "protocol_version": 0,
            "network_id": 0,
            "service_id": 1000,
            "message_id": 0,
            "body": {
                "0": public_key,
                "1": "Carol",
            },
            "signature": message.raw().signature(),
        })
    );
    let message_copy: UnnamedMessage = serde_json::from_value(json).unwrap();
    assert_eq!(message_copy.raw(), message.raw());
}
