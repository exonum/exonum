// Copyright 2020 The Exonum Team
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

//! Handling messages received from P2P node network.
//!
//! Every message passes through three phases:
//!
//!   - `Vec<u8>`: raw bytes as received from the network
//!   - `SignedMessage`: integrity and signature of the message has been verified
//!   - `impl IntoMessage`:  the message has been completely parsed and has correct structure
//!
//! Graphical representation of the message processing flow:
//!
//! ```text
//! +---------+             +---------------+                  +------------------+
//! | Vec<u8> |--(verify)-->| SignedMessage |--(deserialize)-->| impl IntoMessage |-->(handle)
//! +---------+     |       +---------------+        |         +------------------+
//!                 |                                |
//!                 V                                V
//!              (drop)                           (drop)
//!
//! ```
//!
//! # Examples
//!
//! The procedure of creating a new signed message is as follows.
//!
//! ```
//! # use chrono::Utc;
//! # use exonum::{
//! #     crypto::{self, Hash},
//! #     helpers::{Height, Round, ValidatorId},
//! #     messages::{Precommit, Verified},
//! # };
//! # fn send<T>(_: T) {}
//! let keypair = crypto::gen_keypair();
//! // For example, get some `Status` message.
//! let payload = Precommit::new(
//!     ValidatorId(0),
//!     Height(15),
//!     Round::first(),
//!     crypto::hash(b"propose_hash"),
//!     crypto::hash(b"block_hash"),
//!     Utc::now(),
//! );
//! // Sign the message with some keypair to get a trusted `Precommit` message.
//! let signed_payload = Verified::from_value(payload, keypair.0, &keypair.1);
//! // Further, convert the trusted message into a raw signed message and send
//! // it through the network.
//! let raw_signed_message = signed_payload.into_raw();
//! send(raw_signed_message);
//! ```
//!
//! The procedure of verification of a signed message is as follows:
//!
//! ```
//! # use assert_matches::assert_matches;
//! # use chrono::Utc;
//! # use exonum::{
//! #     crypto::{self, Hash},
//! #     helpers::{Height, Round, ValidatorId},
//! #     messages::{CoreMessage, Precommit, Verified, SignedMessage},
//! # };
//! # fn get_signed_message() -> SignedMessage {
//! #     let keypair = crypto::gen_keypair();
//! #     let payload = Precommit::new(
//! #         ValidatorId(0),
//! #         Height(15),
//! #         Round::first(),
//! #         crypto::hash(b"propose_hash"),
//! #         crypto::hash(b"block_hash"),
//! #         Utc::now(),
//! #     );
//! #     Verified::from_value(payload, keypair.0, &keypair.1).into_raw()
//! # }
//! // Assume you have some signed message.
//! let raw: SignedMessage = get_signed_message();
//! // You know that this is a type of `CoreMessage`, so you can
//! // verify its signature and convert it into `CoreMessage`.
//! let verified = raw.into_verified::<CoreMessage>().expect("verification failed");
//! // Further, check whether it is a `Precommit` message.
//! assert_matches!(
//!     verified.payload(),
//!      CoreMessage::Precommit(ref precommit) if precommit.height == Height(15)
//! );
//! ```

pub use self::{
    signed::{IntoMessage, Verified},
    types::*,
};

use crate::crypto::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};

mod signed;
mod types;

/// Lower bound on the size of the correct `SignedMessage`.
/// This is the size of message fields + protobuf overhead.
#[doc(hidden)]
pub const SIGNED_MESSAGE_MIN_SIZE: usize = PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH + 8;

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use exonum_crypto::{self as crypto, gen_keypair};
    use exonum_merkledb::BinaryValue;
    use exonum_proto::ProtobufConvert;
    use protobuf::Message;

    use super::*;
    use crate::{
        helpers::{Height, Round, ValidatorId},
        proto::schema::messages as proto,
    };

    #[test]
    fn test_signed_message_min_size() {
        let (public_key, secret_key) = gen_keypair();
        let msg = SignedMessage::new(vec![], public_key, &secret_key);
        assert_eq!(SIGNED_MESSAGE_MIN_SIZE, msg.into_bytes().len())
    }

    #[test]
    fn test_message_roundtrip() {
        let (pub_key, secret_key) = gen_keypair();
        let ts = Utc::now();

        let msg = Verified::from_value(
            Precommit::new(
                ValidatorId(123),
                Height(15),
                Round(25),
                crypto::hash(&[1, 2, 3]),
                crypto::hash(&[3, 2, 1]),
                ts,
            ),
            pub_key,
            &secret_key,
        );

        let bytes = msg.to_bytes();
        let message =
            SignedMessage::from_bytes(bytes.into()).expect("Cannot deserialize signed message");
        let msg_roundtrip = message
            .into_verified::<Precommit>()
            .expect("Failed to check precommit");
        assert_eq!(msg, msg_roundtrip);
    }

    #[test]
    fn test_signed_message_unusual_protobuf() {
        let (pub_key, secret_key) = gen_keypair();

        let mut ex_msg = proto::CoreMessage::new();
        let precommit_msg = Precommit::new(
            ValidatorId(123),
            Height(15),
            Round(25),
            crypto::hash(&[1, 2, 3]),
            crypto::hash(&[3, 2, 1]),
            Utc::now(),
        );
        ex_msg.set_precommit(precommit_msg.to_pb());
        let mut payload = ex_msg.write_to_bytes().unwrap();
        // Duplicate pb serialization to create unusual but correct protobuf message.
        payload.append(&mut payload.clone());

        let signed = SignedMessage::new(payload, pub_key, &secret_key);

        let bytes = signed.into_bytes();
        let message =
            SignedMessage::from_bytes(bytes.into()).expect("Cannot deserialize signed message");
        let deserialized_precommit = message
            .into_verified::<Precommit>()
            .expect("Failed to check precommit");
        assert_eq!(precommit_msg, *deserialized_precommit.payload())
    }

    #[test]
    fn test_precommit_serde_correct() {
        let (pub_key, secret_key) = gen_keypair();
        let ts = Utc::now();

        let precommit = Verified::from_value(
            Precommit::new(
                ValidatorId(123),
                Height(15),
                Round(25),
                crypto::hash(&[1, 2, 3]),
                crypto::hash(&[3, 2, 1]),
                ts,
            ),
            pub_key,
            &secret_key,
        );

        let precommit_json = serde_json::to_string(&precommit).unwrap();
        let precommit2: Verified<Precommit> = serde_json::from_str(&precommit_json).unwrap();
        assert_eq!(precommit2, precommit);
    }
}
