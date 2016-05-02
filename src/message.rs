use std::{mem, convert, sync};

use byteorder::{ByteOrder, LittleEndian};

use super::crypto::PublicKey;

pub const HEADER_SIZE  : usize = 40;

pub const TEST_NETWORK_ID        : u8 = 0;
pub const PROTOCOL_MAJOR_VERSION : u8 = 0;

pub const PROTOCOL_VERSION : u8 = 0;

pub type Message = sync::Arc<MessageData>;

#[derive(Debug)]
pub struct MessageData {
    data: Vec<u8>,
}

impl MessageData {
    pub fn new() -> MessageData {
        MessageData {
            data: vec![0; HEADER_SIZE]
        }
    }

    pub fn network_id(&self) -> u8 {
        self.data[0]
    }

    pub fn version(&self) -> u8 {
        self.data[1]
    }

    pub fn message_type(&self) -> u16 {
        LittleEndian::read_u16(&self.data[2..4])
    }

    pub fn payload_length(&self) -> usize {
        LittleEndian::read_u32(&self.data[4..8]) as usize
    }

    pub fn public_key(&self) -> &PublicKey {
        unsafe {
            mem::transmute(&self.data[8])
        }
    }

    pub fn set_network_id(&mut self, network_id: u8) {
        self.data[0] = network_id
    }

    pub fn set_version(&mut self, version: u8) {
        self.data[1] = version
    }

    pub fn set_message_type(&mut self, message_type: u16) {
        LittleEndian::write_u16(&mut self.data[2..4], message_type)
    }

    pub fn set_payload_length(&mut self, length: usize) {
        LittleEndian::write_u32(&mut self.data[4..8], length as u32)
    }

    pub fn set_public_key(&mut self, public_key: &PublicKey) {
        let origin : &mut PublicKey = unsafe {
            mem::transmute(&mut self.data[8])
        };
        origin.clone_from(public_key);
    }

    pub fn actual_length(&self) -> usize {
        self.data.len()
    }

    pub fn total_length(&self) -> usize {
        HEADER_SIZE + self.payload_length()
    }

    pub fn header(&self) -> &[u8] {
        unsafe {
            mem::transmute(&self.data[0..HEADER_SIZE])
        }
    }

    pub fn header_mut(&mut self) -> &mut [u8] {
        unsafe {
            mem::transmute(&mut self.data[0..HEADER_SIZE])
        }
    }

    pub fn allocate_payload(&mut self) {
        let size = self.total_length();
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


#[test]
fn test_message_new() {
    let data = MessageData::new();
    assert_eq!(data.network_id(), 0);
    assert_eq!(data.version(), 0);
    assert_eq!(data.message_type(), 0);
    assert_eq!(data.payload_length(), 0);
}

#[test]
fn test_as_mut() {
    let mut data = MessageData::new();
    {
        let bytes = data.as_mut();
        bytes[0] = 1;
        bytes[1] = 2;
        bytes[2] = 3;
        bytes[3] = 0;
        bytes[4] = 5;
        bytes[5] = 6;
    }
    assert_eq!(data.network_id(), 1);
    assert_eq!(data.version(), 2);
    assert_eq!(data.message_type(), 3);
    assert_eq!(data.payload_length(), 1541);
}
