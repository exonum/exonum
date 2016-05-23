use std::{mem, convert, sync};

use byteorder::{ByteOrder, LittleEndian};

use super::super::crypto::{
    PublicKey, SecretKey, Signature,
    sign, verify, Hash, hash, SIGNATURE_LENGTH
};

use super::Error;

pub const HEADER_SIZE : usize = 40; // TODO: rename to HEADER_LENGTH?

pub const TEST_NETWORK_ID        : u8 = 0;
pub const PROTOCOL_MAJOR_VERSION : u8 = 0;

pub type RawMessage = sync::Arc<MessageBuffer>;

// TODO: make sure that message length is enougth when using mem::transmute

#[derive(Debug)]
pub struct MessageBuffer {
    raw: Vec<u8>,
}

impl MessageBuffer {
    pub fn empty() -> MessageBuffer {
        MessageBuffer {
            raw: vec![0; HEADER_SIZE]
        }
    }

    pub fn new(message_type: u16,
               payload_length: usize,
               public_key: &PublicKey) -> MessageBuffer {
        let mut raw = MessageBuffer {
            raw: vec![0; HEADER_SIZE + payload_length]
        };
        raw.set_network_id(TEST_NETWORK_ID);
        raw.set_version(PROTOCOL_MAJOR_VERSION);
        raw.set_message_type(message_type);
        raw.set_payload_length(payload_length);
        raw.set_public_key(public_key);
        raw
    }

    pub fn hash(&self) -> Hash {
        hash(&self.raw)
    }

    pub fn network_id(&self) -> u8 {
        self.raw[0]
    }

    pub fn version(&self) -> u8 {
        self.raw[1]
    }

    pub fn message_type(&self) -> u16 {
        LittleEndian::read_u16(&self.raw[2..4])
    }

    pub fn payload_length(&self) -> usize {
        LittleEndian::read_u32(&self.raw[4..8]) as usize
    }

    pub fn public_key(&self) -> &PublicKey {
        unsafe {
            mem::transmute(&self.raw[8])
        }
    }

    pub fn set_network_id(&mut self, network_id: u8) {
        self.raw[0] = network_id
    }

    pub fn set_version(&mut self, version: u8) {
        self.raw[1] = version
    }

    pub fn set_message_type(&mut self, message_type: u16) {
        LittleEndian::write_u16(&mut self.raw[2..4], message_type)
    }

    pub fn set_payload_length(&mut self, length: usize) {
        LittleEndian::write_u32(&mut self.raw[4..8], length as u32)
    }

    pub fn set_public_key(&mut self, public_key: &PublicKey) {
        let origin : &mut PublicKey = unsafe {
            mem::transmute(&mut self.raw[8])
        };
        origin.clone_from(public_key);
    }

    pub fn actual_length(&self) -> usize {
        self.raw.len()
    }

    pub fn total_length(&self) -> usize {
        HEADER_SIZE + self.payload_length()
    }

    pub fn payload(&self) -> &[u8] {
        &self.raw[HEADER_SIZE..]
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.raw[HEADER_SIZE..]
    }

    pub fn signature(&self) -> &Signature {
        let sign_idx = self.total_length() - SIGNATURE_LENGTH;
        unsafe {
            mem::transmute(&self.raw[sign_idx])
        }
    }

    pub fn signature_mut(&mut self) -> &mut Signature {
        let sign_idx = self.total_length() - SIGNATURE_LENGTH;
        unsafe {
            mem::transmute(&mut self.raw[sign_idx])
        }
    }

    pub fn allocate_payload(&mut self) {
        let size = self.total_length();
        self.raw.resize(size, 0);
    }

    pub fn sign(&mut self, secret_key: &SecretKey) {
        let sign_idx = self.total_length() - SIGNATURE_LENGTH;
        let signature = sign(&self.raw[..sign_idx], secret_key);
        self.signature_mut().clone_from(&signature);
    }

    pub fn verify(&self) -> bool {
        let sign_idx = self.total_length() - SIGNATURE_LENGTH;
        verify(self.signature(), &self.raw[..sign_idx], self.public_key())
    }
}

impl convert::AsRef<[u8]> for MessageBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.raw
    }
}

impl convert::AsMut<[u8]> for MessageBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.raw
    }
}

pub trait Message : Sized {
    const MESSAGE_TYPE : u16;
    const BODY_LENGTH : usize;
    const PAYLOAD_LENGTH : usize;
    const TOTAL_LENGTH : usize;

    fn raw(&self) -> &RawMessage;
    fn from_raw(raw: RawMessage) -> Result<Self, Error>;

    fn verify(&self) -> bool {
        self.raw().verify()
    }
}

#[test]
fn test_empty_message() {
    let raw = MessageBuffer::empty();
    assert_eq!(raw.network_id(), 0);
    assert_eq!(raw.version(), 0);
    assert_eq!(raw.message_type(), 0);
    assert_eq!(raw.payload_length(), 0);
}

#[test]
fn test_as_mut() {
    let mut raw = MessageBuffer::empty();
    {
        let bytes = raw.as_mut();
        bytes[0] = 1;
        bytes[1] = 2;
        bytes[2] = 3;
        bytes[3] = 0;
        bytes[4] = 5;
        bytes[5] = 6;
    }
    assert_eq!(raw.network_id(), 1);
    assert_eq!(raw.version(), 2);
    assert_eq!(raw.message_type(), 3);
    assert_eq!(raw.payload_length(), 1541);
}
