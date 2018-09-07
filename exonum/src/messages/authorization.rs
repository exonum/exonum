use failure::Error;
use hex::{FromHex, ToHex};

use crypto::{
    self, hash, Hash, PublicKey, SecretKey, Signature, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH,
};

use super::EMPTY_SIGNED_MESSAGE_SIZE;

/// Correct raw message that was deserialized and verified, from `UncheckedBuffer`;
///
/// inner data should be formed according to the following layout:
///
/// | Position | Stored data |
/// | - - - - - - - -| - - - - - - |
/// | `0..32`  | author's PublicKey     |
/// | `32`     | message class          |
/// | `33`     | message type           |
/// | `34..N`  | Payload                |
/// | `N..N+64`| Signature              |
///
///
/// Every creation of `SignedMessage` lead to signature verification, or data signing procedure,
/// which can slowdown your code.

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct SignedMessage {
    pub(in messages) raw: Vec<u8>,
}

impl SignedMessage {
    pub(crate) fn new(
        cls: u8,
        tag: u8,
        value: Vec<u8>,
        author: PublicKey,
        secret_key: &SecretKey,
    ) -> SignedMessage {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(author.as_ref());
        buffer.push(cls);
        buffer.push(tag);
        buffer.extend_from_slice(value.as_ref());
        let signature = Self::sign(&buffer, secret_key).expect("Couldn't form signature");
        buffer.extend_from_slice(signature.as_ref());
        SignedMessage { raw: buffer }
    }

    /// Creates `SignedMessage` wrapper from the raw buffer.
    /// Checks binary format and signature.
    pub fn verify_buffer(buffer: Vec<u8>) -> Result<Self, Error> {
        if buffer.len() <= EMPTY_SIGNED_MESSAGE_SIZE {
            bail!("Message too short message_len = {}", buffer.len())
        }
        let signed = SignedMessage { raw: buffer };

        let pk = signed.author();
        let signature = signed.signature();

        Self::verify(signed.data_without_signature(), &signature, &pk)?;

        Ok(signed)
    }

    fn data_without_signature(&self) -> &[u8]{
        debug_assert!(self.raw.len() > EMPTY_SIGNED_MESSAGE_SIZE);
        let sign_idx = self.raw.len() - SIGNATURE_LENGTH;
        &self.raw[0..sign_idx]
    }

    #[cfg(test)]
    pub(crate) fn unchecked_from_vec(buffer: Vec<u8>) -> Self {
        SignedMessage { raw: buffer }
    }

    #[cfg(not(test))]
    pub(in messages) fn unchecked_from_vec(buffer: Vec<u8>) -> Self {
        SignedMessage { raw: buffer }
    }

    pub(in messages) fn author(&self) -> PublicKey {
        PublicKey::from_slice(&self.raw[0..PUBLIC_KEY_LENGTH]).expect("Couldn't read PublicKey")
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

    pub(in messages) fn signature(&self) -> Signature {
        let sign_idx = self.raw.len() - SIGNATURE_LENGTH;
        Signature::from_slice(&self.raw[sign_idx..]).expect("Couldn't read signature")
    }

    /// Return byte array representation of internal data.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    pub fn hash(&self) -> Hash {
        hash(&self.raw)
    }

    fn sign(full_buffer: &[u8], secret_key: &SecretKey) -> Result<Signature, Error> {
        let signature = crypto::sign(&full_buffer, secret_key);
        Ok(signature)
    }

    fn verify(
        full_buffer: &[u8],
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<(), Error> {
        if !crypto::verify(signature, &full_buffer, &public_key) {
            bail!("Can't verify message.");
        }
        Ok(())
    }
}

impl ToHex for SignedMessage {
    fn write_hex<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        self.raw.write_hex(w)
    }

    fn write_hex_upper<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        self.raw.write_hex_upper(w)
    }
}

/// Warning: This implementation checks signature which is slow operation.
impl FromHex for SignedMessage {
    type Error = Error;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<SignedMessage, Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        Self::verify_buffer(bytes)
    }
}
