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

use exonum_merkledb::BinaryValue;
use hex::{FromHex, ToHex};
use serde::{de::{Deserialize, Deserializer}, ser::{Serialize, Serializer}};

use std::{borrow::Cow, convert::TryFrom, fmt, str::FromStr};

use crate::crypto::{self, PublicKey, SecretKey};

use super::types::{ExonumMessage2, SignedMessage};

impl SignedMessage {
    /// Creates a new signed message.
    pub fn new(payload: Cow<[u8]>, author: PublicKey, secret_key: &SecretKey) -> Self {
        let signature = crypto::sign(payload.as_ref(), secret_key);
        SignedMessage {
            payload: payload.into_owned(),
            author,
            signature,
        }
    }

    /// Signs protocol message.
    pub fn from_protocol_msg(
        msg: impl Into<ExonumMessage2>,
        author: PublicKey,
        secret_key: &SecretKey,
    ) -> Self {
        let payload = Cow::Owned(msg.into().into_bytes());
        Self::new(payload, author, secret_key)
    }

    /// Verifies message signature and returns the corresponding checked message.
    pub fn verify<T>(self) -> Result<Verified<T>, failure::Error>
    where
        T: TryFrom<ExonumMessage2>,
    {
        // Verifies message signature
        ensure!(
            crypto::verify(&self.signature, &self.payload, &self.author),
            "Failed to verify signature."
        );
        // Deserializes message.
        let protocol_message = ExonumMessage2::from_bytes(Cow::Borrowed(&self.payload))?;
        let payload = T::try_from(protocol_message)
            .map_err(|_| failure::format_err!("Failed to decode ProtocolMessage."))?;

        Ok(Verified { raw: self, payload })
    }
}

impl ToHex for SignedMessage {
    fn write_hex<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.to_bytes().write_hex(w)
    }

    fn write_hex_upper<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.to_bytes().write_hex_upper(w)
    }
}

impl FromHex for SignedMessage {
    type Error = failure::Error;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Self::Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        Self::from_bytes(bytes.into()).map_err(From::from)
    }
}

impl fmt::Display for SignedMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.write_hex(f)
    }
}

impl FromStr for SignedMessage {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl<'de> Deserialize<'de> for SignedMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde_str::deserialize(deserializer)
    }
}

impl Serialize for SignedMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serde_str::serialize(self, serializer)
    }
}


#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
pub struct Verified<T> {
    raw: SignedMessage,
    payload: T,
}

impl<T> Verified<T> 
where
    T: TryFrom<ExonumMessage2>,
{
    pub fn from_raw<V>(bytes: V) -> Result<Self, failure::Error> 
        where for<'a> V: Into<Cow<'a, [u8]>>
    {
        SignedMessage::from_bytes(bytes.into())?.verify()
    }
}

impl<T> BinaryValue for Verified<T>
where
    T: TryFrom<ExonumMessage2>,
{
    fn to_bytes(&self) -> Vec<u8> {
        self.raw.to_bytes()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let raw = SignedMessage::from_bytes(bytes)?;
        let protocol_message = ExonumMessage2::from_bytes(Cow::Borrowed(&raw.payload))?;
        let payload = T::try_from(protocol_message).map_err(|_| failure::format_err!("Noo"))?;
        Ok(Self { raw, payload })
    }
}

impl<T> AsRef<T> for Verified<T> {
    fn as_ref(&self) -> &T { &self.payload }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::{self, Hash},
        helpers::Height,
        messages::types::{Precommit, Status},
    };

    #[test]
    fn test_verified_from_signed_correct_signature() {
        let keypair = crypto::gen_keypair();

        let msg = Status {
            height: Height(0),
            last_hash: Hash::zero(),
        };
        let protocol_message = ExonumMessage2::from(msg.clone());
        let signed = SignedMessage::from_protocol_msg(protocol_message.clone(), keypair.0, &keypair.1);

        let verified_protocol = signed.clone().verify::<ExonumMessage2>().unwrap();
        assert_eq!(verified_protocol.payload, protocol_message);

        let verified_status = signed.clone().verify::<Status>().unwrap();
        assert_eq!(verified_status.payload, msg);

        // Wrong variant
        let err = signed.verify::<Precommit>().unwrap_err();
        assert_eq!(err.to_string(), "Failed to decode ProtocolMessage.");
    }

    #[test]
    fn test_verified_from_signed_incorrect_signature() {
        let keypair = crypto::gen_keypair();

        let msg = Status {
            height: Height(0),
            last_hash: Hash::zero(),
        };
        let protocol_message = ExonumMessage2::from(msg.clone());
        let mut signed =
            SignedMessage::from_protocol_msg(protocol_message.clone(), keypair.0, &keypair.1);
        // Update author
        signed.author = crypto::gen_keypair().0;
        let err = signed.clone().verify::<ExonumMessage2>().unwrap_err();
        assert_eq!(err.to_string(), "Failed to verify signature.");
    }
}
