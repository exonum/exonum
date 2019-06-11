use exonum_merkledb::{BinaryValue, ObjectHash};
use failure::Error;
use hex::{FromHex, ToHex};
use protobuf::Message;
use serde::de::{self, Deserialize, Deserializer};

use std::{borrow::Cow, fmt};

use crate::{
    crypto::{self, hash, Hash, PublicKey, SecretKey, Signature},
    proto::{self, ProtobufConvert},
};

// FIXME: For the moment, for performance reasons, we have disabled signature verification
// here and we MUST implement [ECR-3272] task to fix possible security vulnerabilities.

/// `SignedMessage` will verify the size of the buffer and the signature provided in it.
/// This allows to keep the raw message buffer, but avoid verifying its signature again
/// as every `SignedMessage` instance is guaranteed to have a correct signature.
#[derive(Serialize, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct SignedMessage {
    exonum_msg: Vec<u8>,
    key: PublicKey,
    sign: Signature,
}

impl SignedMessage {
    // Creates new signed message.
    pub(crate) fn new(value: &[u8], author: PublicKey, secret_key: &SecretKey) -> SignedMessage {
        let signature = crypto::sign(&value, secret_key);
        let msg = SignedMessage {
            exonum_msg: value.to_vec(),
            key: author,
            sign: signature,
        };

        msg.verify().expect("Can't verify signature with given public key.")
    }

    fn from_pb_no_verify(mut pb: <Self as ProtobufConvert>::ProtoStruct) -> Result<Self, Error> {
        Ok(Self {
            exonum_msg: pb.take_exonum_msg(),
            key: ProtobufConvert::from_pb(pb.take_key())?,
            sign: ProtobufConvert::from_pb(pb.take_sign())?,
        })
    }

    /// Verifies message signature.
    pub fn verify(self) -> Result<Self, failure::Error> {
        if !crypto::verify(&self.sign, &self.exonum_msg, &self.key) {
            Err(format_err!("Failed to verify signature."))
        } else {
            Ok(self)
        }
    }

    /// Key which was used to create signature.
    pub fn key(&self) -> &PublicKey {
        &self.key
    }

    /// Signature of the exonum message.
    pub fn signature(&self) -> &Signature {
        &self.sign
    }

    /// Signature of the exonum message.
    #[cfg(test)]
    pub fn signature_mut(&mut self) -> &mut Signature {
        &mut self.sign
    }

    /// Exonum message
    pub fn exonum_message(&self) -> &[u8] {
        &self.exonum_msg
    }
}

impl ProtobufConvert for SignedMessage {
    type ProtoStruct = proto::SignedMessage;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut msg = Self::ProtoStruct::new();
        msg.set_exonum_msg(ProtobufConvert::to_pb(&self.exonum_msg).into());
        msg.set_key(ProtobufConvert::to_pb(&self.key).into());
        msg.set_sign(ProtobufConvert::to_pb(&self.sign).into());
        msg
    }

    /// Warning: This implementation doesn't check signature.
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        let msg = Self::from_pb_no_verify(pb)?;
        Ok(msg)
    }
}

impl BinaryValue for SignedMessage {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_pb().write_to_bytes().unwrap()
    }

    fn from_bytes(value: Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut pb = <Self as ProtobufConvert>::ProtoStruct::new();
        pb.merge_from_bytes(&value)?;
        Self::from_pb(pb)
    }
}

impl ObjectHash for SignedMessage {
    fn object_hash(&self) -> Hash {
        let mut buff = Vec::new();
        buff.extend_from_slice(&self.exonum_msg);
        buff.extend_from_slice(self.key.as_ref());
        buff.extend_from_slice(self.sign.as_ref());
        hash(&buff)
    }
}

/// Purpose of this struct is to use default Deserialize implementation to write our own
/// deserializer which will check signature after deserialization.
#[derive(Deserialize)]
struct SignedMessageDeserializer {
    exonum_msg: Vec<u8>,
    key: PublicKey,
    sign: Signature,
}

impl<'de> Deserialize<'de> for SignedMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let des_msg = SignedMessageDeserializer::deserialize(deserializer)?;
        let msg = SignedMessage {
            exonum_msg: des_msg.exonum_msg,
            key: des_msg.key,
            sign: des_msg.sign,
        };

        msg.verify().map_err(de::Error::custom)
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

/// Warning: This implementation checks signature which is slow operation.
impl FromHex for SignedMessage {
    type Error = Error;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<SignedMessage, Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        Self::from_bytes(bytes.into()).map_err(Error::from).and_then(Self::verify)
    }
}
