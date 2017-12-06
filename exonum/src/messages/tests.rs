// Copyright 2017 The Exonum Team
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

use crypto::{PublicKey, SecretKey, Signature, gen_keypair};
use messages::raw::MessageBuffer;
use messages::RawMessage;
use encoding::serialize::FromHex;

message! {
    struct TxSimple {
        const TYPE = 0;
        const ID = 0;
        const SIZE = 40;
        field public_key: &PublicKey [0 => 32]
        field msg: &str [32 => 40]
    }
}

#[test]
fn test_message_without_fields() {
    message! {
        struct NoFields {
            const TYPE = 0;
            const ID = 0;
            const SIZE = 0;
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
#[should_panic(expected = "IncorrectNetworkId")]
fn test_incorrect_network_id() {
    let tx = TxSimple::new_with_signature(&PublicKey::zero(), "My little pony", &Signature::zero());
    let mut vec = tx.as_ref().as_ref().to_vec();
    vec[0] = 128;
    let _msg = TxSimple::from_raw(RawMessage::from_vec(vec)).unwrap();
}

#[test]
#[allow(dead_code)]
#[should_panic(expected = "Found error in from_raw: UnexpectedlyShortPayload")]
fn test_message_with_small_size() {
    message! {
        struct SmallField {
            const TYPE = 0;
            const ID = 0;
            const SIZE = 1;
            field test: bool [0 => 1]
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
    message! {
        struct TxOtherSize {
            const TYPE = 0;
            const ID = 0;
            const SIZE = 32;
            field public_key: &PublicKey [0 => 32]
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
    message! {
        struct TxOtherBody {
            const TYPE = 0;
            const ID = 0;
            const SIZE = 40;
            field a: u64 [0 => 8]
            field b: u64 [8 => 16]
            field c: u64 [16 => 24]
            field d: u64 [24 => 32]
            field e: u64 [32 => 40]
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
    message! {
        struct TxOtherId {
            const TYPE = 0;
            const ID = 1;
            const SIZE = 40;
            field public_key: &PublicKey [0 => 32]
            field msg: &str [32 => 40]
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
    message! {
        struct TxOtherType {
            const TYPE = 1;
            const ID = 0;
            const SIZE = 40;
            field public_key: &PublicKey [0 => 32]
            field msg: &str [32 => 40]
        }
    }
    let keypair = gen_keypair();
    let msg = TxSimple::new(&keypair.0, "I am a simple!", &keypair.1);
    let hex = msg.to_hex();
    let _msg = TxOtherType::from_hex(hex).unwrap();
}
