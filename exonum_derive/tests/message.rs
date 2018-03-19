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

extern crate exonum;
#[macro_use]
extern crate exonum_derive;
#[macro_use]
extern crate serde_json;

use exonum::crypto::{self, PublicKey};
use exonum::messages::{Message, RawMessage, Write};
use exonum::storage::StorageValue;
use exonum::encoding::serialize::{FromHex, encode_hex};

#[derive(Debug, MessageSet)]
#[exonum(service_id = "100")]
pub enum Transactions<'a> {
    CreateWallet {
        public_key: &'a PublicKey,
        name: &'a str,
    },

    Transfer {
        from: &'a PublicKey,
        to: &'a PublicKey,
        amount: u64,
        seed: u64,
    },
}

/// Owned version of `Transactions`. This is the type that `Message` will be implemented for.
#[derive(Clone, Message)]
#[exonum(payload = "Transactions")]
struct MyMessage(RawMessage);

#[test]
fn test_message() {
    let (public_key, secret_key) = crypto::gen_keypair();

    let raw: RawMessage = Transactions::CreateWallet {
        public_key: &public_key,
        name: "Alice",
    }.sign(&secret_key);

    assert!(raw.verify_signature(&public_key));

    let message = MyMessage::from_raw(raw).unwrap();
    assert_eq!(message.raw().service_id(), 100);
    assert_eq!(message.raw().message_type(), 0);

    match message.payload() {
        Transactions::CreateWallet {
            public_key: pk,
            name,
        } => {
            assert_eq!(*pk, public_key);
            assert_eq!(name, "Alice");
        }
        v => panic!("Unexpected variant: {:?}", v),
    }

    let json = serde_json::to_value(&message).unwrap();
    assert_eq!(
        json,
        json!({
            "protocol_version": 0,
            "network_id": 0,
            "service_id": 100,
            "message_id": 0,
            "body": {
                "public_key": public_key,
                "name": "Alice",
            },
            "signature": message.raw().signature(),
        })
    );

    let message_copy: MyMessage = serde_json::from_value(json).unwrap();
    assert_eq!(message_copy.raw(), message.raw());

    let bytes = message.clone().into_bytes();
    let message_copy = MyMessage::from_bytes(bytes.into());
    assert_eq!(message_copy.raw(), message.raw());

    let hex = encode_hex(message.raw().as_ref());
    let message_copy = MyMessage::from_hex(hex).unwrap();
    assert_eq!(message_copy.raw(), message.raw());
}
