use failure::Error;
use hex::{FromHex, ToHex};

use std::fmt;

use super::EMPTY_SIGNED_MESSAGE_SIZE;
use crate::crypto::{
    self, hash, Hash, PublicKey, SecretKey, Signature, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH,
};
use crate::messages::BinaryForm;
use crate::proto::{self, ProtobufConvert};
use crate::storage::StorageValue;
use exonum_crypto::CryptoHash;
use protobuf::Message;
use serde::de::{self, Deserialize, Deserializer};
use std::borrow::Cow;

/// `SignedMessage` can be constructed from a raw byte buffer which must have the following
/// data layout:
///
/// | Position  | Stored data             |
/// | - - - - - | - - - - - - - - - - - - |
/// | `0..32`   | author's public key     |
/// | `32`      | message class           |
/// | `33`      | message type            |
/// | `34..N`   | payload                 |
/// | `N..N+64` | signature               |
///
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

        if msg.verify() {
            msg
        } else {
            panic!("Can't verify signature with given public key.");
        }
    }

    fn from_pb_no_verify(mut pb: <Self as ProtobufConvert>::ProtoStruct) -> Result<Self, Error> {
        Ok(Self {
            exonum_msg: pb.take_exonum_msg(),
            key: ProtobufConvert::from_pb(pb.take_key())?,
            sign: ProtobufConvert::from_pb(pb.take_sign())?,
        })
    }

    fn verify(&self) -> bool {
        crypto::verify(&self.sign, &self.exonum_msg, &self.key)
    }

    /// Key which was used to create signature.
    pub fn key(&self) -> &PublicKey {
        &self.key
    }

    /// Signature of the exonum message.
    pub fn signature(&self) -> &Signature {
        &self.sign
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

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        let msg = Self::from_pb_no_verify(pb)?;
        ensure!(msg.verify(), "Failed to verify signature.");
        Ok(msg)
    }
}

impl BinaryForm for SignedMessage {
    fn encode(&self) -> std::result::Result<Vec<u8>, Error> {
        self.to_pb().write_to_bytes().map_err(Error::from)
    }

    fn decode(buffer: &[u8]) -> std::result::Result<Self, Error> {
        let mut pb = <Self as ProtobufConvert>::ProtoStruct::new();
        pb.merge_from_bytes(buffer)?;
        Self::from_pb(pb)
    }
}

impl StorageValue for SignedMessage {
    fn into_bytes(self) -> Vec<u8> {
        self.encode().unwrap()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        let mut pb = <Self as ProtobufConvert>::ProtoStruct::new();
        pb.merge_from_bytes(&value).unwrap();
        Self::from_pb_no_verify(pb).unwrap()
    }
}

impl CryptoHash for SignedMessage {
    fn hash(&self) -> Hash {
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
        let deser_msg = SignedMessageDeserializer::deserialize(deserializer)?;
        let msg = SignedMessage {
            exonum_msg: deser_msg.exonum_msg,
            key: deser_msg.key,
            sign: deser_msg.sign,
        };

        if msg.verify() {
            Ok(msg)
        } else {
            Err(de::Error::custom(format_err!(
                "Can't verify message signature"
            )))
        }
    }
}

impl ToHex for SignedMessage {
    fn write_hex<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.encode()
            .map_err(|_| std::fmt::Error)
            .and_then(|v| v.write_hex(w))
    }

    fn write_hex_upper<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.encode()
            .map_err(|_| std::fmt::Error)
            .and_then(|v| v.write_hex_upper(w))
    }
}

/// Warning: This implementation checks signature which is slow operation.
impl FromHex for SignedMessage {
    type Error = Error;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<SignedMessage, Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        Self::decode(&bytes).map_err(Error::from)
    }
}
