use std::{mem, convert};

use super::signature::PublicKey;

pub const HEADER_SIZE  : usize = 40;
pub const MESSAGE_SIZE : usize = 64;

pub const TEST_NETWORK_ID        : u8 = 0;
pub const PROTOCOL_MAJOR_VERSION : u8 = 0;

#[derive(Debug)]
pub struct Message {
    pub header: MessageHeader,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
#[repr(packed)]
pub struct MessageHeader {
    network_id   : u8,
    version      : u8,
    service_id   : u8,
    message_type : u8,
    length       : u32,
    public_key   : PublicKey,
}

impl convert::AsRef<[u8]> for MessageHeader {
    fn as_ref(&self) -> &[u8] {
        let bytes : &[u8; HEADER_SIZE] = unsafe {
            mem::transmute(self)
        };
        bytes
    }
}

impl convert::AsMut<[u8]> for MessageHeader {
    fn as_mut(&mut self) -> &mut [u8] {
        let bytes : &mut [u8; HEADER_SIZE] = unsafe {
            mem::transmute(self)
        };
        bytes
    }
}

// FIXME: big endian vs little endian

impl MessageHeader {
    pub fn new() -> MessageHeader {
        unsafe {
            mem::zeroed()
        }
    }

    pub fn network_id(&self) -> u8 {
        self.network_id
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn service_id(&self) -> u8 {
        self.service_id
    }

    pub fn message_type(&self) -> u8 {
        self.message_type
    }

    pub fn length(&self) -> usize {
        self.length as usize
    }

    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    pub fn set_network_id(&mut self, network_id: u8) {
        self.network_id = network_id
    }

    pub fn set_version(&mut self, version: u8) {
        self.version = version
    }

    pub fn set_service_id(&mut self, service_id: u8) {
        self.service_id = service_id
    }

    pub fn set_message_type(&mut self, message_type: u8) {
        self.message_type = message_type
    }

    pub fn set_length(&mut self, length: usize) {
        self.length = length as u32
    }

    pub fn set_public_key(&mut self, public_key: &PublicKey) {
        self.public_key = public_key.clone()
    }
}

impl Message {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Message {
        Message {
            header: header,
            data: data
        }
    }
}

#[test]
fn test_sizes() {
    assert_eq!(::std::mem::size_of::<MessageHeader>(), HEADER_SIZE);
    assert_eq!(::std::mem::size_of::<Message>(), MESSAGE_SIZE);
}

#[test]
fn test_header_new() {
    let header = MessageHeader::new();
    assert_eq!(header.network_id(), 0);
    assert_eq!(header.version(), 0);
    assert_eq!(header.service_id(), 0);
    assert_eq!(header.message_type(), 0);
    assert_eq!(header.length(), 0);
}

#[test]
fn test_as_mut() {
    let mut header = MessageHeader::new();
    {
        let bytes = header.as_mut();
        bytes[0] = 1;
        bytes[1] = 2;
        bytes[2] = 3;
        bytes[3] = 4;
        bytes[4] = 5;
        bytes[5] = 6;
    }
    assert_eq!(header.network_id(), 1);
    assert_eq!(header.version(), 2);
    assert_eq!(header.service_id(), 3);
    assert_eq!(header.message_type(), 4);
    assert_eq!(header.length(), 1541);
}
