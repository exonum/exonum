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

use crypto::{gen_keypair, PublicKey, SecretKey, Signature};
use encoding::serialize::FromHex;
use messages::{raw::MessageBuffer, Message, RawMessage, RawTransaction};

messages! {
    const SERVICE_ID = 0;

    struct TxSimple {
        public_key: &PublicKey,
        msg: &str,
    }
}

#[test]
fn test_debug_transaction() {
    let (p_key, s_key) = gen_keypair();
    let tx = TxSimple::new(&p_key, "Hello, World!", &s_key);
    let vec = tx.raw().as_ref().to_vec();
    let transaction: RawTransaction = RawTransaction::from_vec(vec);

    let debug = format!("{:?}", transaction);
    let expected = format!(
        "Transaction {{ version: {:?}, service_id: {:?}, message_type: {:?}, length: {:?}, hash: {:?} }}",
        transaction.version(),
        transaction.service_id(),
        transaction.message_type(),
        transaction.len(),
        transaction.hash()
    );

    assert_eq!(debug, expected)
}

#[test]
fn test_message_without_fields() {
    messages! {
        const SERVICE_ID = 0;

        struct NoFields {
        }
    }
    drop(NoFields::new(&SecretKey::new([1; 64])));
}

#[test]
#[should_panic(expected = "UnsupportedProtocolVersion")]
fn test_unsupported_version() {
    let tx = TxSimple::new_with_signature(&PublicKey::zero(), "My little pony", &Signature::zero());
    let mut vec = tx.as_ref().as_ref().to_vec();
    vec[1] = 128;
    let _msg = TxSimple::from_raw(RawMessage::from_vec(vec)).unwrap();
}

#[test]
#[allow(dead_code)]
#[should_panic(expected = "Found error in from_raw: UnexpectedlyShortPayload")]
fn test_message_with_small_size() {
    messages! {
        const SERVICE_ID = 0;
        struct SmallField {
            test: bool,
        }
    }

    let buf = vec![1; 1];
    let raw = RawMessage::new(MessageBuffer::from_vec(buf));
    let _message = SmallField::from_raw(raw).expect("Found error in from_raw");
}

#[test]
fn test_hex_valid_into_message() {
    let keypair = gen_keypair();
    let msg = TxSimple::new(&keypair.0, "I am a simple!", &keypair.1);
    let hex = msg.to_hex();
    let msg2 = TxSimple::from_hex(hex).expect("Unable to decode hex into `TxFirst`");
    assert_eq!(msg2, msg);
}

#[test]
#[allow(dead_code)]
#[should_panic(expected = "Incorrect raw message length")]
fn test_hex_wrong_length_into_message() {
    messages! {
        const SERVICE_ID = 0;
        struct TxOtherSize {
            public_key: &PublicKey,
        }
    }
    let keypair = gen_keypair();
    let msg = TxSimple::new(&keypair.0, "I am a simple!", &keypair.1);
    let hex = msg.to_hex();
    let _msg = TxOtherSize::from_hex(hex).unwrap();
}

#[test]
#[allow(dead_code)]
#[should_panic(expected = "OverlappingSegment")]
fn test_hex_wrong_body_into_message() {
    messages! {
        const SERVICE_ID = 0;
        struct TxOtherBody {
            a: u64,
            b: u64,
            c: u64,
            d: u64,
            e: u64,
        }
    }
    let msg = TxOtherBody::new_with_signature(0, 1, 2, 3, 4, &Signature::zero());
    let hex = msg.to_hex();
    let _msg = TxSimple::from_hex(hex).unwrap();
}

#[test]
#[allow(dead_code)]
#[should_panic(expected = "IncorrectMessageType")]
fn test_hex_wrong_id_into_message() {
    messages! {
        const SERVICE_ID = 0;
        struct MessageWithZeroId {
        }

        struct TxOtherId {
            public_key: &PublicKey,
            msg: &str,
        }
    }
    let keypair = gen_keypair();
    let msg = TxSimple::new(&keypair.0, "I am a simple!", &keypair.1);
    let hex = msg.to_hex();
    let _msg = TxOtherId::from_hex(hex).unwrap();
}

#[test]
#[allow(dead_code)]
#[should_panic(expected = "IncorrectServiceId")]
fn test_hex_wrong_type_into_message() {
    messages! {
        const SERVICE_ID = 1;
        struct TxOtherType {
            public_key: &PublicKey,
            msg: &str,
        }
    }
    let keypair = gen_keypair();
    let msg = TxSimple::new(&keypair.0, "I am a simple!", &keypair.1);
    let hex = msg.to_hex();
    let _msg = TxOtherType::from_hex(hex).unwrap();
}
