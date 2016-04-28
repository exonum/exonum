use std::{mem, convert, sync};

use super::signature::PublicKey;

pub const HEADER_SIZE  : usize = 40;
pub const MESSAGE_SIZE : usize = 8;

pub const TEST_NETWORK_ID        : u8 = 0;
pub const PROTOCOL_MAJOR_VERSION : u8 = 0;

pub const PROTOCOL_VERSION : u8 = 0;

pub type Message = sync::Arc<MessageData>;

#[derive(Debug)]
pub struct MessageData {
    data: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
#[repr(packed)]
pub struct MessageHeader {
    network_id   : u8,
    version      : u8,
    message_type : u16,
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

impl MessageData {
    pub fn new() -> MessageData {
        MessageData {
            data: vec![0; HEADER_SIZE]
        }
    }

    pub fn actual_length(&self) -> usize {
        self.data.len()
    }

    pub fn total_length(&self) -> usize {
        HEADER_SIZE + self.header().length()
    }

    pub fn header(&self) -> &MessageHeader {
        unsafe {
            mem::transmute(&self.data[0])
        }
    }

    pub fn header_mut(&mut self) -> &mut MessageHeader {
        unsafe {
            mem::transmute(&mut self.data[0])
        }
    }

    pub fn allocate_payload(&mut self) {
        let size = HEADER_SIZE + self.header().length();
        self.data.resize(size, 0);
    }

    pub fn extend(&mut self, data: &[u8]) {
        self.data.extend(data);
    }

    pub fn payload(&self) -> &[u8] {
        &self.data[HEADER_SIZE..]
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.data[HEADER_SIZE..]
    }
}

impl convert::AsRef<[u8]> for MessageData {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl convert::AsMut<[u8]> for MessageData {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

// FIXME: big endian vs little endian

impl MessageHeader {
    pub fn new() -> MessageHeader {
        unsafe {
            mem::zeroed()
        }
    }

    // TODO: move all this methods to MessageData type

    pub fn network_id(&self) -> u8 {
        self.network_id
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn message_type(&self) -> u16 {
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

    pub fn set_message_type(&mut self, message_type: u16) {
        self.message_type = message_type
    }

    pub fn set_length(&mut self, length: usize) {
        self.length = length as u32
    }

    pub fn set_public_key(&mut self, public_key: &PublicKey) {
        self.public_key = public_key.clone()
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
        bytes[3] = 0;
        bytes[4] = 5;
        bytes[5] = 6;
    }
    assert_eq!(header.network_id(), 1);
    assert_eq!(header.version(), 2);
    assert_eq!(header.message_type(), 3);
    assert_eq!(header.length(), 1541);
}
