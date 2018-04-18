use failure::Error;
use byteorder::{ByteOrder, LittleEndian};
use bincode;
use serde::{Serialize, Deserialize};

use crypto::{self, hash, CryptoHash, Hash, PublicKey, SecretKey, Signature,
             SIGNATURE_LENGTH, PUBLIC_KEY_LENGTH};

use super::protocol::{Protocol, ProtocolMessage};
use super::{ROTOCOL_MAJOR_VERSION, MAX_MESSAGE_SIZE};

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
struct AuthorisedMessage {
    reserved: u8,
    version: u8,
    author: PublicKey,
    payload: Protocol
}

impl AuthorisedMessage {
    fn new<T: Into<Protocol>>(value: T, author: PublicKey) -> Result<Self, Error> {
        Ok(AuthorisedMessage {
            reserved: 0,
            version: PROTOCOL_MAJOR_VERSION,
            author,
            payload: value.into(),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SignedMessage {
    authorised_message: AuthorisedMessage,
    signature: Signature,
}

impl SignedMessage {
    pub fn new<Into<Protocol>>(value: T,
                                               service_id: u16,
                                               author: PublicKey,
                                               secret_key: &SecretKey)
                                               -> Result<SignedMessage, Error> {
        let authorised_message = AuthorisedMessage::new(value, author)?;
        let signature = Self::sign(authorised_message);

        Ok(SignedMessage {
            authorised_message,
            signature,
        })
    }

    pub(crate) fn verify_buffer(buffer: &[u8]) -> Result<SignedMessage, Error> {
        // TODO: external serialization library shadows any knowledge about internal
        // binary representation.
        // Sodium verify/sign api allows to work only with raw buffer.
        // This two factors lead to additional `serialize` inside verify
        let message: SignedMessage = bincode::deserialize(&*buffer)?;
        if !crypto::verify(message.signature,
                           buffer,
                           &message.authorised_message.author) {
            bail!("Can't verify message.");
        }
        Ok(message)
    }

    pub(crate) fn into_buffer(self) -> Result<Vec<u8>, Error> {
        bincode::serialize(&val, Bounded(MAX_MESSAGE_SIZE))?
    }

    fn sign<T: Serialize>(val: &T) -> Signature {
        let full_buffer = bincode::serialize(&val, Bounded(MAX_MESSAGE_SIZE))?;
        let signature = crypto::sign_detached(&full_buffer, secret_key);
        signature
    }

}
