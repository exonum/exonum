use std::borrow::Cow;

use failure::Error;
use byteorder::{ByteOrder, LittleEndian};
use bincode::Config;
use serde::{Serialize, Deserialize};

use crypto::{self, hash, CryptoHash, Hash, PublicKey, SecretKey, Signature,
             SIGNATURE_LENGTH, PUBLIC_KEY_LENGTH};
use messages::Message;
use storage::StorageValue;

use super::protocol::{Protocol, ProtocolMessage};
use super::{PROTOCOL_MAJOR_VERSION, MAX_MESSAGE_SIZE};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AuthorisedMessage {
    pub reserved: u8,
    pub version: u8,
    pub author: PublicKey,
    pub protocol: Protocol
}

impl AuthorisedMessage {
    fn new<T: Into<Protocol>>(value: T, author: PublicKey) -> Result<Self, Error> {
        Ok(AuthorisedMessage {
            reserved: 0,
            version: PROTOCOL_MAJOR_VERSION,
            author,
            protocol: value.into(),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SignedMessage {
    pub(crate) authorised_message: AuthorisedMessage,
    pub(crate) signature: Signature,
}

impl SignedMessage {
    pub fn new<T: Into<Protocol>>(value: T,
                                       author: PublicKey,
                                       secret_key: &SecretKey)
                                       -> Result<SignedMessage, Error> {
        let authorised_message = AuthorisedMessage::new(value, author)?;
        let signature = Self::sign(&authorised_message, secret_key)?;

        Ok(SignedMessage {
            authorised_message,
            signature,
        })
    }

    pub fn verify_buffer<T: AsRef<[u8]>>(buffer: T) -> Result<SignedMessage, Error> {
        // TODO: external serialization library shadows any knowledge about internal
        // binary representation.
        // Sodium verify/sign api allows to work only with raw buffer.
        // This two factors lead to additional `serialize` inside verify
        let buffer = buffer.as_ref();
        let message: SignedMessage = ::bincode::config().no_limit().deserialize(&buffer)?;
        Self::verify(&message.authorised_message,
                     &message.signature,
                    &message.authorised_message.author)?;
        Ok(message)
    }

    pub fn into_buffer(self) -> Result<Vec<u8>, Error> {
        Ok(::bincode::config().no_limit().serialize(&self)?)
    }

    pub fn to_message(self) -> Message {
        Message {
            payload: self.authorised_message.protocol.clone(),
            message: self
        }
    }

    fn sign<T: Serialize>(val: &T, secret_key: &SecretKey) -> Result<Signature, Error> {
        let full_buffer = ::bincode::config().no_limit().serialize(&val)?;
        let signature = crypto::sign(&full_buffer, secret_key);
        Ok(signature)
    }

    fn hash(&self) -> Hash {
        hash(&::bincode::config()
                .no_limit()
                .serialize(self)
                .expect("Expected serialize to work"))
    }

    fn verify<T: Serialize>(val: &T, signature: &Signature, public_key: &PublicKey) -> Result<(), Error> {
        let full_buffer = ::bincode::config().no_limit().serialize(&val)?;
        if !crypto::verify(signature,
                           &full_buffer,
                           &public_key) {
            bail!("Can't verify message.");
        }
        Ok(())
    }

}


impl StorageValue for SignedMessage {
    fn into_bytes(self) -> Vec<u8> {
        self.into_buffer().expect("Serialisation failed")
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        SignedMessage::verify_buffer(&value).unwrap()
    }
}

impl CryptoHash for SignedMessage {
    fn hash(&self) -> Hash {
        self.hash()
    }
}

impl<T: ProtocolMessage> StorageValue for Message<T> {
    fn into_bytes(self) -> Vec<u8> {
        unimplemented!()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        unimplemented!()
    }
}

impl<T: ProtocolMessage> CryptoHash for Message<T> {
    fn hash(&self) -> Hash {
        unimplemented!()
    }
}
