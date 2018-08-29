use std::borrow::Cow;

use bincode;
use failure::Error;
use serde::Serialize;

use crypto::{self, hash, CryptoHash, Hash, PublicKey, SecretKey, Signature,
             SIGNATURE_LENGTH, PUBLIC_KEY_LENGTH};
use messages::Message;
use storage::StorageValue;
use hex::{FromHex, ToHex};

use super::protocol::{Protocol, ProtocolMessage};
use super::PROTOCOL_MAJOR_VERSION;

use encoding::serialize::encode_hex;

/// Correct raw message that was deserialized and verifyied, from `UncheckedBuffer`;
/// inner data should be formed according to the following layout:
/// | Position | Stored data |
/// | - - - - - - - -| - - - - - - |
/// | `0..32`  | author's PublicKey     |
/// | `32`     | message class          |
/// | `33`     | message type           |
/// | `34..N`  | Payload                |
/// | `N..N+64`| Signature                |
///
///
/// Every creation of `SignedMessage` lead to signature verification, or data signing procedure,
/// which can slowdown your code. Beware `SignedMessage` message, this procedure is not free.


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct SignedMessage {
    raw: Vec<u8>,
}

impl SignedMessage {

    pub(crate) fn new<T>(
        value: T,
        author: PublicKey,
        secret_key: &SecretKey,
    ) -> SignedMessage {
        unimplemented!()
//        let authorized_message = AuthorizedMessage::new(value, author)?;
//        let signature = Self::sign(&authorized_message, secret_key);
//
//        SignedMessage {
//            authorized_message,
//            signature,
//        })
    }

    /// Create `SignedMessage` wrapper from raw buffer.
    /// Checks binary format and signature.
    pub fn verify_buffer(buffer: Vec<u8>) -> Result<Self, Error> {
        unimplemented!();
//
//        if message.authorized_message.version != PROTOCOL_MAJOR_VERSION {
//            bail!(
//                "Message version differ from our supported, msg_version = {}",
//                message.authorized_message.version
//            )
//        }
//        Self::verify(
//            &message.authorized_message,
//            &message.signature,
//            &message.authorized_message.author,
//        )?;

//        Ok(message)
    }

    #[allow(unsafe_code)]
    pub(in messages) fn author(&self) -> &PublicKey {
        unsafe { &*(&self.raw[0] as *const u8 as *const PublicKey) }
    }

    pub(in messages) fn message_class(&self) -> u8 {
        self.raw[PUBLIC_KEY_LENGTH]
    }

    pub(in messages) fn message_type(&self) -> u8 {
        self.raw[PUBLIC_KEY_LENGTH + 1]
    }


    pub(in messages) fn payload(&self) -> &[u8] {
        let sign_idx = self.raw.len() - SIGNATURE_LENGTH;
        &self.raw[PUBLIC_KEY_LENGTH + 2..sign_idx]
    }

    #[allow(unsafe_code)]
    pub(in messages) fn signature(&self) -> &Signature {
        let sign_idx = self.raw.len() - SIGNATURE_LENGTH;
        unsafe { &*(&self.raw[sign_idx] as *const u8 as *const Signature) }
    }

    /// Return byte array representation of internal data.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    fn sign<T: Serialize>(val: &T, secret_key: &SecretKey) -> Result<Signature, Error> {
        unimplemented!();
//        // TODO: limit bincode max_message_length using config
//        let full_buffer = bincode::config().no_limit().serialize(&val)?;
//        let signature = crypto::sign(&full_buffer, secret_key);
//        Ok(signature)
    }

    fn verify<T: Serialize>(
        val: &T,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<(), Error> {
        unimplemented!();
//        let full_buffer = bincode::config().no_limit().serialize(&val)?;
//        if !crypto::verify(signature, &full_buffer, &public_key) {
//            bail!("Can't verify message.");
//        }
        Ok(())
    }
}

impl ToHex for SignedMessage {
    fn write_hex<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        self.raw().write_hex(w)
    }

    fn write_hex_upper<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        self.raw().write_hex_upper(w)
    }
}

// Warning: This implementation checks signature
impl FromHex for SignedMessage {
    type Error = Error;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<SignedMessage, Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        Self::verify_buffer(bytes)
    }
}

//
//
//impl<T: ProtocolMessage> StorageValue for Message<T> {
//    fn into_bytes(self) -> Vec<u8> {
//        self.message.to_vec()
//    }
//
//    fn from_bytes(value: Cow<[u8]>) -> Self {
//        //TODO: Remove signature checks and type checks (Getting value from database should be safe)
//        let message = SignedMessage::verify_buffer(&value).unwrap().into_message();
//        message.map_into().unwrap()
//    }
//}
//
//impl<T: ProtocolMessage> CryptoHash for Message<T> {
//    fn hash(&self) -> Hash {
//        self.hash()
//    }
//}
