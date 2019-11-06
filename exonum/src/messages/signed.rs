// Copyright 2019 The Exonum Team
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

use exonum_merkledb::{BinaryValue, ObjectHash};
use serde::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, Serializer},
};

use std::{borrow::Cow, convert::TryFrom};

use crate::crypto::{self, Hash, PublicKey, SecretKey};

use super::types::{ExonumMessage, SignedMessage};

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
    pub(super) raw: SignedMessage,
    pub(super) inner: T,
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

impl<T> Verified<T>
where
    T: TryFrom<SignedMessage> + Into<ExonumMessage> + TryFrom<ExonumMessage>,
{
    /// Signs the specified value and creates a new verified message from it.
    pub fn from_value(inner: T, public_key: PublicKey, secret_key: &SecretKey) -> Self {
        // Curious trick to avoid clone.
        // Converts inner to the `ExonumMessage` type to proper serialization.
        let exonum_msg = inner.into();
        let raw = SignedMessage::new(exonum_msg.to_bytes(), public_key, secret_key);
        // Converts back to the inner type.
        let inner = if let Ok(inner) = T::try_from(exonum_msg) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::{self, Hash},
        helpers::Height,
        messages::types::{ExonumMessage, Precommit, Status},
        runtime::{AnyTx, CallInfo},
    };

    #[test]
    fn test_verified_from_signed_correct_signature() {
        let keypair = crypto::gen_keypair();

        let msg = Status {
            height: Height(0),
            last_hash: Hash::zero(),
            pool_size: 0,
        };
        let protocol_message = ExonumMessage::from(msg.clone());
        let signed = SignedMessage::new(protocol_message.clone(), keypair.0, &keypair.1);

        let verified_protocol = signed.clone().into_verified::<ExonumMessage>().unwrap();
        assert_eq!(verified_protocol.inner, protocol_message);

        let verified_status = signed.clone().into_verified::<Status>().unwrap();
        assert_eq!(verified_status.inner, msg);

        // Wrong variant
        let err = signed.into_verified::<Precommit>().unwrap_err();
        assert_eq!(err.to_string(), "Failed to decode message from payload.");
    }

    #[test]
    fn test_verified_from_signed_incorrect_signature() {
        let keypair = crypto::gen_keypair();

        let msg = Status {
            height: Height(0),
            last_hash: Hash::zero(),
            pool_size: 0,
        };
        let protocol_message = ExonumMessage::from(msg.clone());
        let mut signed = SignedMessage::new(protocol_message.clone(), keypair.0, &keypair.1);
        // Update author
        signed.author = crypto::gen_keypair().0;
        let err = signed.clone().into_verified::<ExonumMessage>().unwrap_err();
        assert_eq!(err.to_string(), "Failed to verify signature.");
    }

    #[test]
    fn test_verified_status_binary_value() {
        let keypair = crypto::gen_keypair();

        let msg = Verified::from_value(
            Status {
                height: Height(0),
                last_hash: Hash::zero(),
                pool_size: 0,
            },
            keypair.0,
            &keypair.1,
        );
        assert_eq!(msg.object_hash(), msg.as_raw().object_hash());

        let bytes = msg.to_bytes();
        let msg2 = Verified::<Status>::from_bytes(bytes.into()).unwrap();
        assert_eq!(msg, msg2);
    }

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
}
