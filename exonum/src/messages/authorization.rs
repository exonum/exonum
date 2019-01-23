use failure::Error;
use hex::{FromHex, ToHex};

use std::fmt;

use super::EMPTY_SIGNED_MESSAGE_SIZE;
use crate::crypto::{
    self, hash, Hash, PublicKey, SecretKey, Signature, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH,
};

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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct SignedMessage {
    pub(in crate::messages) raw: Vec<u8>,
}

impl SignedMessage {
    /// Creates `SignedMessage` from parts.
    pub(crate) fn new(
        class: u8,
        tag: u8,
        value: &[u8],
        author: PublicKey,
        secret_key: &SecretKey,
    ) -> SignedMessage {
        let mut buffer = Vec::with_capacity(2 + value.len() + PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH);
        buffer.extend_from_slice(author.as_ref());
        buffer.push(class);
        buffer.push(tag);
        buffer.extend_from_slice(value);
        let signature = Self::sign(&buffer, secret_key).expect("Couldn't form signature");
        buffer.extend_from_slice(signature.as_ref());
        SignedMessage { raw: buffer }
    }

    /// Creates `SignedMessage` from parts with specific signature.
    #[cfg(test)]
    pub(crate) fn new_with_signature(
        class: u8,
        tag: u8,
        value: &[u8],
        author: PublicKey,
        signature: Signature,
    ) -> SignedMessage {
        let mut buffer = Vec::with_capacity(2 + value.len() + PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH);
        buffer.extend_from_slice(author.as_ref());
        buffer.push(class);
        buffer.push(tag);
        buffer.extend_from_slice(value);
        buffer.extend_from_slice(signature.as_ref());
        SignedMessage { raw: buffer }
    }

    /// Creates `SignedMessage` wrapper from the raw buffer.
    /// Checks binary format and signature.
    pub fn from_raw_buffer(buffer: Vec<u8>) -> Result<Self, Error> {
        ensure!(
            buffer.len() > EMPTY_SIGNED_MESSAGE_SIZE,
            "Message too short message_len = {}",
            buffer.len()
        );
        let signed = SignedMessage { raw: buffer };

        let pk = signed.author();
        let signature = signed.signature();

        Self::verify(signed.data_without_signature(), &signature, &pk)?;

        Ok(signed)
    }

    fn data_without_signature(&self) -> &[u8] {
        debug_assert!(self.raw.len() > EMPTY_SIGNED_MESSAGE_SIZE);
        let sign_idx = self.raw.len() - SIGNATURE_LENGTH;
        &self.raw[0..sign_idx]
    }

    /// Creates `SignedMessage` from buffer, didn't verify buffer size nor signature.
    pub(crate) fn from_vec_unchecked(buffer: Vec<u8>) -> Self {
        SignedMessage { raw: buffer }
    }

    /// Returns `PublicKey` of message author.
    pub(in crate::messages) fn author(&self) -> PublicKey {
        PublicKey::from_slice(&self.raw[0..PUBLIC_KEY_LENGTH]).expect("Couldn't read PublicKey")
    }

    /// Returns message class, which is an ID inside protocol.
    pub(in crate::messages) fn message_class(&self) -> u8 {
        self.raw[PUBLIC_KEY_LENGTH]
    }

    /// Returns message type, which is an ID inside some class of messages.
    pub(in crate::messages) fn message_type(&self) -> u8 {
        self.raw[PUBLIC_KEY_LENGTH + 1]
    }

    /// Returns serialized payload of the message.
    pub(in crate::messages) fn payload(&self) -> &[u8] {
        let sign_idx = self.raw.len() - SIGNATURE_LENGTH;
        &self.raw[PUBLIC_KEY_LENGTH + 2..sign_idx]
    }

    /// Returns ed25519 signature for this message.
    pub(in crate::messages) fn signature(&self) -> Signature {
        let sign_idx = self.raw.len() - SIGNATURE_LENGTH;
        Signature::from_slice(&self.raw[sign_idx..]).expect("Couldn't read signature")
    }

    /// Returns byte array representation of internal data.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Calculates a hash of inner data.
    pub fn hash(&self) -> Hash {
        hash(&self.raw)
    }

    /// Signs buffer with `secret_key`.
    /// This method returns ed25519 signature.
    fn sign(full_buffer: &[u8], secret_key: &SecretKey) -> Result<Signature, Error> {
        let signature = crypto::sign(&full_buffer, secret_key);
        Ok(signature)
    }

    /// Verifies buffer integrity, and authenticate buffer.
    fn verify(
        full_buffer: &[u8],
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<(), Error> {
        if !crypto::verify(signature, &full_buffer, &public_key) {
            bail!("Cannot verify message.");
        }
        Ok(())
    }
}

impl ToHex for SignedMessage {
    fn write_hex<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.raw.write_hex(w)
    }

    fn write_hex_upper<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.raw.write_hex_upper(w)
    }
}

/// Warning: This implementation checks signature which is slow operation.
impl FromHex for SignedMessage {
    type Error = Error;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<SignedMessage, Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        Self::from_raw_buffer(bytes)
    }
}
