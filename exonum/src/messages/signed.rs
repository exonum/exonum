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

use exonum_merkledb::{impl_serde_hex_for_binary_value, BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use failure::{ensure, Error};
use serde::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, Serializer},
};

use std::{
    borrow::Cow,
    convert::{TryFrom, TryInto},
};

use crate::{
    crypto::{self, Hash, PublicKey, SecretKey, Signature},
    messages::types::SignedMessage,
    proto,
};

impl SignedMessage {
    /// Creates a new signed message from the given binary value.
    pub fn new(payload: impl BinaryValue, author: PublicKey, secret_key: &SecretKey) -> Self {
        let payload = payload.into_bytes();
        let signature = crypto::sign(payload.as_ref(), secret_key);
        Self {
            payload,
            author,
            signature,
        }
    }

    /// Verifies message signature and returns the corresponding checked message.
    pub fn into_verified<T>(self) -> Result<Verified<T>, failure::Error>
    where
        T: TryFrom<Self>,
    {
        // Verifies message signature
        ensure!(
            crypto::verify(&self.signature, &self.payload, &self.author),
            "Failed to verify signature."
        );
        // Deserializes message.
        let inner = T::try_from(self.clone())
            .map_err(|_| failure::format_err!("Failed to decode message from payload."))?;

        Ok(Verified { raw: self, inner })
    }
}

impl_serde_hex_for_binary_value! { SignedMessage }

/// Wraps a `Payload` together with the corresponding `SignedMessage`.
///
/// Usually one wants to work with fully parsed and verified messages (i.e., `Payload`).
/// However, occasionally we have to retransmit the message into the network or
/// save its serialized form. We could serialize the `Payload` back,
/// but Protobuf does not have a canonical form so the resulting payload may
/// have different binary representation (thus invalidating the message signature).
///
/// So we use `Verified` to keep the original byte buffer around with the parsed `Payload`.
///
/// Be careful with `BinaryValue::from_bytes` method!
/// It for performance reasons skips signature verification.
///
/// See module [documentation](index.html#examples) for examples.
#[derive(Clone, Debug)]
pub struct Verified<T> {
    raw: SignedMessage,
    inner: T,
}

impl<T> PartialEq for Verified<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw.eq(&other.raw)
    }
}

impl<T> Verified<T> {
    /// Returns reference to the underlying signed message.
    pub fn as_raw(&self) -> &SignedMessage {
        &self.raw
    }

    /// Takes the underlying signed message.
    pub fn into_raw(self) -> SignedMessage {
        self.raw
    }

    /// Returns message author key.
    pub fn author(&self) -> PublicKey {
        self.raw.author
    }

    /// Downcasts this message to a more specific type. This is only appropriate if the target
    /// type retains all information about the message.
    pub fn downcast_map<U>(self, map_fn: impl FnOnce(T) -> U) -> Verified<U>
    where
        U: TryFrom<SignedMessage> + IntoMessage,
    {
        Verified {
            raw: self.raw,
            inner: map_fn(self.inner),
        }
    }
}

impl<T> Verified<T>
where
    T: TryFrom<SignedMessage>,
{
    /// Returns reference to the underlying message payload.
    pub fn payload(&self) -> &T {
        &self.inner
    }

    /// Takes the underlying message payload.
    pub fn into_payload(self) -> T {
        self.inner
    }
}

/// Message that can be converted into a unambiguous presentation for signing. "Unambiguous"
/// means that any sequence of bytes produced by serializing `Container` obtained by converting
/// this message can be interpreted in a single way. In other words, messages of different types
/// have separated representation domains.
pub trait IntoMessage: Sized {
    /// Container for the message.
    type Container: BinaryValue + From<Self> + TryInto<Self>;
}

impl<T> Verified<T>
where
    T: TryFrom<SignedMessage> + IntoMessage,
{
    /// Signs the specified value and creates a new verified message from it.
    pub fn from_value(inner: T, public_key: PublicKey, secret_key: &SecretKey) -> Self {
        let container: T::Container = inner.into();
        let raw = SignedMessage::new(container.to_bytes(), public_key, secret_key);
        // Converts back to the inner type.
        let inner: T = if let Ok(inner) = container.try_into() {
            inner
        } else {
            unreachable!("We can safely convert `ExonumMessage` back to the inner type.")
        };
        Self { raw, inner }
    }
}

impl<'de, T> Deserialize<'de> for Verified<T>
where
    T: TryFrom<SignedMessage>,
{
    /// Warning: This implementation checks signature which is slow operation.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        SignedMessage::deserialize(deserializer)?
            .into_verified::<T>()
            .map_err(serde::de::Error::custom)
    }
}

impl<T> Serialize for Verified<T>
where
    T: TryFrom<SignedMessage>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_raw().serialize(serializer)
    }
}

impl<T> BinaryValue for Verified<T>
where
    for<'a> T: TryFrom<&'a SignedMessage>,
{
    fn to_bytes(&self) -> Vec<u8> {
        self.raw.to_bytes()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, failure::Error> {
        let raw = SignedMessage::from_bytes(bytes)?;
        let inner =
            T::try_from(&raw).map_err(|_| failure::format_err!("Unable to decode message"))?;
        Ok(Self { raw, inner })
    }
}

impl<T> ObjectHash for Verified<T>
where
    for<'a> T: TryFrom<&'a SignedMessage>,
{
    fn object_hash(&self) -> Hash {
        self.raw.object_hash()
    }
}

impl<T> AsRef<T> for Verified<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> From<Verified<T>> for SignedMessage {
    fn from(msg: Verified<T>) -> Self {
        msg.into_raw()
    }
}

impl<T> ProtobufConvert for Verified<T>
where
    T: TryFrom<SignedMessage>,
{
    type ProtoStruct = proto::SignedMessage;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut message = Self::ProtoStruct::new();
        let signed_message = self.as_raw();

        message.set_payload(signed_message.payload.to_pb());
        message.set_author(signed_message.author.to_pb());
        message.set_signature(signed_message.signature.to_pb());
        message
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, Error> {
        let signed_message = SignedMessage {
            payload: pb.take_payload(),
            author: PublicKey::from_pb(pb.take_author())?,
            signature: Signature::from_pb(pb.take_signature())?,
        };

        signed_message.into_verified()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use exonum_crypto::{self as crypto, Signature};
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        helpers::{Height, Round, ValidatorId},
        messages::Precommit,
        runtime::{AnyTx, CallInfo},
    };

    #[test]
    fn test_verified_any_tx_binary_value() {
        let keypair = crypto::gen_keypair();

        let msg = Verified::from_value(
            AnyTx {
                call_info: CallInfo {
                    instance_id: 5,
                    method_id: 2,
                },
                arguments: vec![1, 2, 3, 4],
            },
            keypair.0,
            &keypair.1,
        );
        assert_eq!(msg.object_hash(), msg.as_raw().object_hash());

        let bytes = msg.to_bytes();
        let msg2 = Verified::<AnyTx>::from_bytes(bytes.into()).unwrap();
        assert_eq!(msg, msg2);
    }

    #[test]
    fn test_verified_protobuf_convert() {
        let keypair = crypto::gen_keypair();

        let msg = Verified::from_value(
            AnyTx {
                call_info: CallInfo {
                    instance_id: 5,
                    method_id: 2,
                },
                arguments: vec![1, 2, 3, 4],
            },
            keypair.0,
            &keypair.1,
        );

        let to_pb = msg.to_pb();
        let from_pb = Verified::from_pb(to_pb).expect("Failed to convert from protobuf.");

        assert_eq!(msg, from_pb);
    }

    #[test]
    #[should_panic(expected = "Failed to verify signature.")]
    fn test_precommit_serde_wrong_signature() {
        let (pub_key, secret_key) = crypto::gen_keypair();
        let ts = Utc::now();

        let mut precommit = Verified::from_value(
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
        // Break signature.
        precommit.raw.signature = Signature::zero();

        let precommit_json = serde_json::to_string(&precommit).unwrap();
        let precommit2: Verified<Precommit> = serde_json::from_str(&precommit_json).unwrap();
        assert_eq!(precommit2, precommit);
    }
}
