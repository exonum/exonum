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
use super::{Message, ProtocolMessage};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UncheckedBuffer {
    message: Vec<u8>,
}

impl UncheckedBuffer {
    pub fn new(vec: Vec<u8>) -> UncheckedBuffer {
        UncheckedBuffer { message: vec }
    }
    pub fn get_vec(&self) -> &Vec<u8> {
        &self.message
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
    pub fn from_vec(raw: Vec<u8>) -> Self {
        // TODO: Check that size >= HEADER_LENGTH. (ECR-166)
        // TODO: Check that payload_length == raw.len(). (ECR-166)
        Self { raw }
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
        &self.message
    }
}

impl<T: ProtocolMessage> From<Message<T>> for UncheckedBuffer {
    fn from(val: Message<T>) -> UncheckedBuffer {
        UncheckedBuffer::new(val.into_parts().1.to_vec())
    }
}
