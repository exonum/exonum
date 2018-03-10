// Copyright 2017 The Exonum Team
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

//! Helpers for `exonum_derive`.

use serde::Serialize;
use serde_json::Value;

use std::error::Error as StdError;

use crypto::Signature;
use encoding;
use messages::{Message, MessageBuffer, MessageWriter, RawMessage};

/// Helper object for message (de)serialization.
#[derive(Debug, Serialize)]
pub struct MessageStub<T> {
    /// Protocol version specified in the message.
    pub protocol_version: u8,
    /// Network identifier specified in the message.
    pub network_id: u8,
    /// Service identifier.
    pub service_id: u16,
    /// Message identifier.
    pub message_id: u16,
    /// Message payload.
    pub body: T,
    /// Message signature.
    pub signature: Signature,
}

/// Alias for message stubs used during deserialization.
pub type DeStub<'a> = MessageStub<&'a Value>;

impl<T> MessageStub<T> {
    /// Writes itself into a raw message.
    pub fn write<F>(
        &self,
        header_size: usize,
        mut payload_writer: F,
    ) -> Result<RawMessage, encoding::Error>
    where
        F: FnMut(&mut MessageWriter) -> Result<(), encoding::Error>,
    {
        let mut writer = MessageWriter::new(
            self.protocol_version,
            self.network_id,
            self.service_id,
            self.message_id,
            header_size,
        );
        payload_writer(&mut writer)?;
        Ok(RawMessage::new(writer.append_signature(&self.signature)))
    }
}

impl<T: Serialize> MessageStub<T> {
    /// Constructs a new instance of the message stub.
    pub fn new(raw: &RawMessage, payload: T) -> Self {
        MessageStub {
            protocol_version: raw.version(),
            network_id: raw.network_id(),
            service_id: raw.service_id(),
            message_id: raw.message_type(),
            body: payload,
            signature: *raw.signature(),
        }
    }
}

impl<'a> MessageStub<&'a Value> {
    /// Parses the message stub from a specified JSON value.
    pub fn from_value(value: &'a Value) -> Result<Self, Box<StdError>> {
        use serde_json::from_value;

        let obj = value.as_object().ok_or("Can't cast json as object.")?;

        let body = obj.get("body").ok_or("Can't get body from json.")?;
        let signature = from_value(
            obj.get("signature")
                .ok_or("Can't get signature from json")?
                .clone(),
        )?;
        let message_id = from_value(
            obj.get("message_id")
                .ok_or("Can't get message_id from json")?
                .clone(),
        )?;
        let service_id = from_value(
            obj.get("service_id")
                .ok_or("Can't get service_id from json")?
                .clone(),
        )?;
        let network_id = from_value(
            obj.get("network_id")
                .ok_or("Can't get network_id from json")?
                .clone(),
        )?;
        let protocol_version = from_value(
            obj.get("protocol_version")
                .ok_or("Can't get protocol_version from json")?
                .clone(),
        )?;

        Ok(MessageStub {
            protocol_version,
            network_id,
            service_id,
            message_id,
            body,
            signature,
        })
    }
}

/// Tries to parse a message from hex.
pub fn message_from_hex<T: Message, S: AsRef<[u8]>>(hex: S) -> Result<T, encoding::Error> {
    use hex::FromHex;

    let vec = Vec::<u8>::from_hex(hex).map_err(|e| {
        encoding::Error::Other(Box::new(e))
    })?;

    if vec.len() < ::messages::HEADER_LENGTH {
        return Err(encoding::Error::Basic("Hex is too short.".into()));
    }

    let buf = MessageBuffer::from_vec(vec);
    let raw = RawMessage::new(buf);
    Message::from_raw(raw)
}
