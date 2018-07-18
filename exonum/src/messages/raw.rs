// Copyright 2018 The Exonum Team
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

#![allow(unsafe_code)]

use byteorder::{ByteOrder, LittleEndian};

use std::{convert, fmt::Debug, ops::Deref, sync};

use crypto::{
    hash, sign, verify, CryptoHash, Hash, PublicKey, SecretKey, Signature, SIGNATURE_LENGTH,
};
use encoding::{self, CheckedOffset, Field, Offset, Result as StreamStructResult};

/// Length of the message header.
pub const HEADER_LENGTH: usize = 10;
/// Version of the protocol. Different versions are incompatible.
pub const PROTOCOL_MAJOR_VERSION: u8 = 0;

/// Thread-safe reference-counting pointer to the `MessageBuffer`.
#[derive(Clone, PartialEq)]
pub struct RawMessage(sync::Arc<MessageBuffer>);

impl RawMessage {
    /// Creates a new `RawMessage` instance with the given `MessageBuffer`.
    pub fn new(buffer: MessageBuffer) -> Self {
        RawMessage(sync::Arc::new(buffer))
    }

    /// Creates a new `RawMessage` instance from the given `Vec<u8>`.
    pub fn from_vec(vec: Vec<u8>) -> Self {
        RawMessage(sync::Arc::new(MessageBuffer::from_vec(vec)))
    }

    /// Returns hash of the `RawMessage`.
    pub fn hash(&self) -> Hash {
        hash(self.as_ref())
    }
}

impl Deref for RawMessage {
    type Target = MessageBuffer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for RawMessage {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref().as_ref()
    }
}

// TODO: Reduce `to` argument from `write`, `read` and `check` methods. (ECR-166)
// TODO: Payload_length as a first value into message header. (ECR-166)
// TODO: Make sure that message length is enough when using mem::transmute. (ECR-166)

/// A raw message represented by the bytes buffer.
#[derive(Debug, PartialEq)]
pub struct MessageBuffer {
    raw: Vec<u8>,
}

impl MessageBuffer {
    /// Creates `MessageBuffer` instance from the bytes vector.
    ///
    /// # Example
    ///
    /// ```
    /// use exonum::messages::MessageBuffer;
    ///
    /// let message_buffer = MessageBuffer::from_vec(vec![1, 2, 3]);
    /// assert!(!message_buffer.is_empty());
    /// ```
    pub fn from_vec(raw: Vec<u8>) -> MessageBuffer {
        // TODO: Check that size >= HEADER_LENGTH. (ECR-166)
        // TODO: Check that payload_length == raw.len(). (ECR-166)
        MessageBuffer { raw }
    }

    /// Returns the length of the message in bytes.
    ///
    /// # Example
    ///
    /// ```
    /// use exonum::messages::MessageBuffer;
    ///
    /// let data = vec![1, 2, 3];
    /// let message_buffer = MessageBuffer::from_vec(data.clone());
    /// assert_eq!(data.len(), message_buffer.len());
    /// ```
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    /// Returns `true` if the `MessageBuffer` contains no bytes.
    ///
    /// # Example
    ///
    /// ```
    /// use exonum::messages::MessageBuffer;
    ///
    /// let message_buffer = MessageBuffer::from_vec(vec![]);
    /// assert!(message_buffer.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// Returns the protocol version.
    pub fn version(&self) -> u8 {
        self.raw[1]
    }

    /// Returns id of the service.
    pub fn service_id(&self) -> u16 {
        LittleEndian::read_u16(&self.raw[4..6])
    }

    /// Returns type of the message.
    pub fn message_type(&self) -> u16 {
        LittleEndian::read_u16(&self.raw[2..4])
    }

    /// Returns message body without signature.
    pub fn body(&self) -> &[u8] {
        &self.raw[..self.raw.len() - SIGNATURE_LENGTH]
    }

    /// Returns signature of the message.
    pub fn signature(&self) -> &Signature {
        let sign_idx = self.raw.len() - SIGNATURE_LENGTH;
        unsafe { &*(&self.raw[sign_idx] as *const u8 as *const Signature) }
    }

    /// Checks that `Field` can be safely got with specified `from` and `to` offsets.
    pub fn check<'a, F: Field<'a>>(
        &'a self,
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> StreamStructResult {
        F::check(
            self.body(),
            (from + HEADER_LENGTH as u32)?,
            (to + HEADER_LENGTH as u32)?,
            latest_segment,
        )
    }

    /// Returns `Field` specified by `from` and `to` offsets. Should not be used directly.
    pub unsafe fn read<'a, F: Field<'a>>(&'a self, from: Offset, to: Offset) -> F {
        F::read(
            self.body(),
            from + HEADER_LENGTH as u32,
            to + HEADER_LENGTH as u32,
        )
    }
}

impl convert::AsRef<[u8]> for MessageBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.raw
    }
}

/// Message writer.
#[derive(Debug, PartialEq)]
pub struct MessageWriter {
    raw: Vec<u8>,
}

impl MessageWriter {
    /// Creates a `MessageWriter` instance with given parameters.
    pub fn new(
        protocol_version: u8,
        service_id: u16,
        message_type: u16,
        payload_length: usize,
    ) -> Self {
        // First byte is reserved for backward-compatibility and better alignment.
        let mut raw = MessageWriter {
            raw: vec![0; HEADER_LENGTH + payload_length],
        };
        raw.set_version(protocol_version);
        raw.set_service_id(service_id);
        raw.set_message_type(message_type);
        raw
    }

    /// Sets version.
    fn set_version(&mut self, version: u8) {
        self.raw[1] = version
    }

    /// Sets the service id.
    fn set_service_id(&mut self, service_id: u16) {
        LittleEndian::write_u16(&mut self.raw[4..6], service_id)
    }

    /// Sets the message type.
    fn set_message_type(&mut self, message_type: u16) {
        LittleEndian::write_u16(&mut self.raw[2..4], message_type)
    }

    /// Sets the length of the payload.
    fn set_payload_length(&mut self, length: usize) {
        LittleEndian::write_u32(&mut self.raw[6..10], length as u32)
    }

    /// Writes given field to the given offset.
    #[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
    pub fn write<'a, F: Field<'a>>(&'a mut self, field: F, from: Offset, to: Offset) {
        field.write(
            &mut self.raw,
            from + HEADER_LENGTH as Offset,
            to + HEADER_LENGTH as Offset,
        );
    }

    /// Signs the message with the given secret key.
    pub fn sign(mut self, secret_key: &SecretKey) -> MessageBuffer {
        let payload_length = self.raw.len() + SIGNATURE_LENGTH;
        self.set_payload_length(payload_length);
        let signature = sign(&self.raw, secret_key);
        self.raw.extend_from_slice(signature.as_ref());
        MessageBuffer { raw: self.raw }
    }

    /// Appends the given signature to the message.
    pub fn append_signature(mut self, signature: &Signature) -> MessageBuffer {
        let payload_length = self.raw.len() + SIGNATURE_LENGTH;
        self.set_payload_length(payload_length);
        self.raw.extend_from_slice(signature.as_ref());
        debug_assert_eq!(self.raw.len(), payload_length);
        MessageBuffer { raw: self.raw }
    }
}

// This is a trait that is required for technical reasons:
// you can't make associated constants object-safe. This
// limitation of the Rust language might be lifted in the
// future.
/// A `Message` which belongs to a particular service.
pub trait ServiceMessage: Message {
    /// ID of the service this message belongs to.
    const SERVICE_ID: u16;
    /// ID of the message itself. Should be unique
    /// within a service.
    const MESSAGE_ID: u16;
}

/// Represents generic message interface.
///
/// An Exonum message is a piece of data that is signed by the creator's [Ed25519] key;
/// the resulting digital signature is a part of the message.
///
/// [Ed25519]: ../crypto/index.html
pub trait Message: CryptoHash + Debug + Send + Sync {
    /// Converts the raw message into the specific one.
    fn from_raw(raw: RawMessage) -> Result<Self, encoding::Error>
    where
        Self: Sized;

    /// Returns raw message.
    fn raw(&self) -> &RawMessage;

    /// Verifies the message using given public key.
    fn verify_signature(&self, pub_key: &PublicKey) -> bool {
        self.raw().verify_signature(pub_key)
    }
}

impl<T: Message> CryptoHash for T {
    fn hash(&self) -> Hash {
        hash(self.raw().as_ref())
    }
}

impl Message for RawMessage {
    fn from_raw(raw: RawMessage) -> Result<Self, encoding::Error> {
        Ok(raw)
    }

    fn raw(&self) -> &RawMessage {
        self
    }

    fn verify_signature(&self, pub_key: &PublicKey) -> bool {
        verify(self.signature(), self.body(), pub_key)
    }
}
