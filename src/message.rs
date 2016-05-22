use std::{mem, convert, sync};
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};

use time::{Timespec};
use byteorder::{ByteOrder, LittleEndian};

use super::crypto::{
    PublicKey, SecretKey, Signature,
    sign, verify, Hash, hash, SIGNATURE_LENGTH
};

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

pub trait ProtocolMessage {
    const MESSAGE_TYPE : u16;
    const BODY_LENGTH : usize;
    const PAYLOAD_LENGTH : usize;
    const TOTAL_LENGTH : usize;

    fn raw(&self) -> &RawMessage;
    fn from_raw(raw: RawMessage) -> Self;

    fn verify(&self) -> bool {
        self.raw().verify()
    }
}

pub trait MessageField<'a> {
    // TODO: use Read and Cursor
    // TODO: debug_assert_eq!(to-from == size of Self)
    fn read(buffer: &'a [u8], from: usize, to: usize) -> Self;
    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize);
}

impl<'a> MessageField<'a> for u32 {
    fn read(buffer: &'a [u8], from: usize, to: usize) -> u32 {
        LittleEndian::read_u32(&buffer[from..to])
    }

    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize) {
        LittleEndian::write_u32(&mut buffer[from..to], *self)
    }
}

impl<'a> MessageField<'a> for u64 {
    fn read(buffer: &'a [u8], from: usize, to: usize) -> u64 {
        LittleEndian::read_u64(&buffer[from..to])
    }

    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize) {
        LittleEndian::write_u64(&mut buffer[from..to], *self)
    }
}

impl<'a> MessageField<'a> for &'a Hash {
    fn read(buffer: &'a [u8], from: usize, _: usize) -> &'a Hash {
        unsafe {
            mem::transmute(&buffer[from])
        }
    }

    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize) {
        &mut buffer[from..to].copy_from_slice(self.as_ref());
    }
}

impl<'a> MessageField<'a> for Timespec {
    fn read(buffer: &'a [u8], from: usize, to: usize) -> Timespec {
        let nsec = LittleEndian::read_u64(&buffer[from..to]);
        Timespec {
            sec:  (nsec / 1_000_000_000) as i64,
            nsec: (nsec % 1_000_000_000) as i32,
        }
    }

    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize) {
        let nsec = (self.sec as u64) * 1_000_000_000 + self.nsec as u64;
        LittleEndian::write_u64(&mut buffer[from..to], nsec)
    }
}

impl<'a> MessageField<'a> for SocketAddr {
    // TODO: supporting IPv6

    fn read(buffer: &'a [u8], from: usize, to: usize) -> SocketAddr {
        let ip = Ipv4Addr::new(buffer[from+0], buffer[from+1],
                               buffer[from+2], buffer[from+3]);
        let port = LittleEndian::read_u16(&buffer[from+4..to]);
        SocketAddr::V4(SocketAddrV4::new(ip, port))
    }

    fn write(&self, buffer: &'a mut [u8], from: usize, to: usize) {
        match *self {
            SocketAddr::V4(addr) => {
                &mut buffer[from..to-2].copy_from_slice(&addr.ip().octets());
            },
            SocketAddr::V6(_) => {
                // FIXME: Supporting Ipv6
                panic!("Ipv6 are currently unsupported")
            },
        }
        LittleEndian::write_u16(&mut buffer[to-2..to], self.port());
    }
}

#[macro_export]
macro_rules! message {
    ($name:ident {
        const ID = $id:expr;
        const SIZE = $body:expr;

        $($field_name:ident : $field_type:ty [$from:expr => $to:expr])*
    }) => (
        #[derive(Clone)]
        pub struct $name {
            raw: $crate::message::RawMessage
        }

        impl $crate::message::ProtocolMessage for $name {
            const MESSAGE_TYPE : u16 = $id;
            const BODY_LENGTH : usize = $body;
            const PAYLOAD_LENGTH : usize =
                $body + $crate::crypto::SIGNATURE_LENGTH;
            const TOTAL_LENGTH : usize =
                $body + $crate::crypto::SIGNATURE_LENGTH
                      + $crate::message::HEADER_SIZE;

            fn raw(&self) -> &$crate::message::RawMessage {
                &self.raw
            }

            fn from_raw(raw: $crate::message::RawMessage) -> $name {
                $name { raw: raw }
            }
        }

        impl $name {
            pub fn new($($field_name: $field_type,)*
                       public_key: &$crate::crypto::PublicKey,
                       secret_key: &$crate::crypto::SecretKey) -> $name {
                use $crate::message::{
                    RawMessage, MessageBuffer, ProtocolMessage, MessageField
                };
                let mut raw = MessageBuffer::new(Self::MESSAGE_TYPE,
                                              Self::PAYLOAD_LENGTH,
                                              public_key);
                {
                    let mut payload = raw.payload_mut();
                    $($field_name.write(&mut payload, $from, $to);)*
                }
                raw.sign(secret_key);
                $name::from_raw(RawMessage::new(raw))
            }
            $(pub fn $field_name(&self) -> $field_type {
                use $crate::message::MessageField;
                <$field_type>::read(self.raw.payload(), $from, $to)
            })*
        }
    )
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
