use std::{mem, net};

use byteorder::{ByteOrder, LittleEndian};
use time::Timespec;

use super::crypto::{Hash, PublicKey, SecretKey, SIGNATURE_LENGTH};
use super::message::{Message, RawMessage, ProtocolMessage};

// CONNECT MESSAGE

pub struct Connect<'a> {
    raw: &'a [u8]
}

impl<'a> ProtocolMessage<'a> for Connect<'a> {
    const MESSAGE_TYPE   : u16   = 0;
    const PAYLOAD_LENGTH : usize = 18 + SIGNATURE_LENGTH;

    fn from_raw(raw: &'a RawMessage) -> Self {
        if raw.message_type() != Self::MESSAGE_TYPE {
            panic!("Trying to read message with incorrect reader type");
        }
        Connect { raw: raw.payload() }
    }
}

impl<'a> Connect<'a> {
    pub fn new(socket_address: &net::SocketAddr,
               time: Timespec,
               public_key: &PublicKey,
               secret_key: &SecretKey) -> Message {
        let mut raw = Connect::raw(public_key);
        {
            let mut payload = raw.payload_mut();
            match *socket_address {
                net::SocketAddr::V4(addr) => {
                    &mut payload[0..4].copy_from_slice(&addr.ip().octets());
                },
                net::SocketAddr::V6(_) => {
                    // FIXME: Supporting Ipv6
                    panic!("Ipv6 are currently unsupported")
                },
            }
            LittleEndian::write_u16(&mut payload[4..6], socket_address.port());
            LittleEndian::write_i64(&mut payload[06..14], time.sec);
            LittleEndian::write_i32(&mut payload[14..18], time.nsec);
        }
        raw.sign(secret_key);
        Message::new(raw)
    }

    pub fn socket_address(&self) -> net::SocketAddr {
        let ip = net::Ipv4Addr::new(self.raw[0], self.raw[1],
                                    self.raw[2], self.raw[3]);
        let port = LittleEndian::read_u16(&self.raw[4..6]);
        net::SocketAddr::V4(net::SocketAddrV4::new(ip, port))
    }

    pub fn time(&self) -> Timespec {
        Timespec {
            sec:  LittleEndian::read_i64(&self.raw[6..14]),
            nsec: LittleEndian::read_i32(&self.raw[14..18]),
        }
    }
}

// PROPOSE MESSAGE

pub struct Propose<'a> {
    raw: &'a [u8]
}

impl<'a> ProtocolMessage<'a> for Propose<'a> {
    const MESSAGE_TYPE   : u16   = 1;
    const PAYLOAD_LENGTH : usize = 56 + SIGNATURE_LENGTH;

    fn from_raw(raw: &'a RawMessage) -> Self {
        if raw.message_type() != Self::MESSAGE_TYPE {
            panic!("Trying to read message with incorrect reader type");
        }
        Propose { raw: raw.payload() }
    }
}

impl<'a> Propose<'a> {
    pub fn new(height: u64,
               round: u32,
               time: Timespec,
               prev_hash: &Hash,
               public_key: &PublicKey,
               secret_key: &SecretKey) -> Message {
        let mut raw = Propose::raw(public_key);
        {
            let mut payload = raw.payload_mut();
            LittleEndian::write_u64(&mut payload[00..08], height);
            LittleEndian::write_u32(&mut payload[08..12], round);
            LittleEndian::write_i64(&mut payload[12..20], time.sec);
            LittleEndian::write_i32(&mut payload[20..24], time.nsec);
            &mut payload[24..56].copy_from_slice(prev_hash.as_ref());
        }
        raw.sign(secret_key);
        Message::new(raw)
    }

    pub fn height(&self) -> u64 {
        LittleEndian::read_u64(&self.raw[0..8])
    }

    pub fn round(&self) -> u32 {
        LittleEndian::read_u32(&self.raw[8..12])
    }

    pub fn time(&self) -> Timespec {
        Timespec {
            sec:  LittleEndian::read_i64(&self.raw[12..20]),
            nsec: LittleEndian::read_i32(&self.raw[20..24]),
        }
    }

    pub fn prev_hash(&self) -> &Hash {
        unsafe {
            mem::transmute(&self.raw[24])
        }
    }
}

// PREVOTE MESSAGE

pub struct Prevote<'a> {
    raw: &'a [u8]
}

impl<'a> ProtocolMessage<'a> for Prevote<'a> {
    const MESSAGE_TYPE   : u16   = 2;
    const PAYLOAD_LENGTH : usize = 44 + SIGNATURE_LENGTH;

    fn from_raw(raw: &'a RawMessage) -> Self {
        if raw.message_type() != Self::MESSAGE_TYPE {
            panic!("Trying to read message with incorrect reader type");
        }
        Prevote { raw: raw.payload() }
    }
}

impl<'a> Prevote<'a> {
    pub fn new(height: u64,
               round: u32,
               hash: &Hash,
               public_key: &PublicKey,
               secret_key: &SecretKey) -> Message {
        let mut raw = Prevote::raw(public_key);
        {
            let mut payload = raw.payload_mut();
            LittleEndian::write_u64(&mut payload[0..8], height);
            LittleEndian::write_u32(&mut payload[8..12], round);
            &mut payload[12..44].copy_from_slice(hash.as_ref());
        }
        raw.sign(secret_key);
        Message::new(raw)
    }

    pub fn height(&self) -> u64 {
        LittleEndian::read_u64(&self.raw[0..8])
    }

    pub fn round(&self) -> u32 {
        LittleEndian::read_u32(&self.raw[8..12])
    }

    pub fn hash(&self) -> &Hash {
        unsafe {
            mem::transmute(&self.raw[12])
        }
    }
}

// PRECOMMIT MESSAGE

pub struct Precommit<'a> {
    raw: &'a [u8]
}

impl<'a> ProtocolMessage<'a> for Precommit<'a> {
    const MESSAGE_TYPE   : u16   = 3;
    const PAYLOAD_LENGTH : usize = 44 + SIGNATURE_LENGTH;

    fn from_raw(raw: &'a RawMessage) -> Self {
        if raw.message_type() != Self::MESSAGE_TYPE {
            panic!("Trying to read message with incorrect reader type");
        }
        Precommit { raw: raw.payload() }
    }
}

impl<'a> Precommit<'a> {
    pub fn new(height: u64,
               round: u32,
               hash: &Hash,
               public_key: &PublicKey,
               secret_key: &SecretKey) -> Message {
        let mut raw = Precommit::raw(public_key);
        {
            let mut payload = raw.payload_mut();
            LittleEndian::write_u64(&mut payload[0..8], height);
            LittleEndian::write_u32(&mut payload[8..12], round);
            &mut payload[12..44].copy_from_slice(hash.as_ref());
        }
        raw.sign(secret_key);
        Message::new(raw)
    }

    pub fn height(&self) -> u64 {
        LittleEndian::read_u64(&self.raw[0..8])
    }

    pub fn round(&self) -> u32 {
        LittleEndian::read_u32(&self.raw[8..12])
    }

    pub fn hash(&self) -> &Hash {
        unsafe {
            mem::transmute(&self.raw[12])
        }
    }
}

// COMMIT MESSAGE

pub struct Commit<'a> {
    raw: &'a [u8]
}

impl<'a> ProtocolMessage<'a> for Commit<'a> {
    const MESSAGE_TYPE   : u16   = 4;
    const PAYLOAD_LENGTH : usize = 40 + SIGNATURE_LENGTH;

    fn from_raw(raw: &'a RawMessage) -> Self {
        if raw.message_type() != Self::MESSAGE_TYPE {
            panic!("Trying to read message with incorrect reader type");
        }
        Commit { raw: raw.payload() }
    }
}

impl<'a> Commit<'a> {
    pub fn new(height: u64,
               hash: &Hash,
               public_key: &PublicKey,
               secret_key: &SecretKey) -> Message {
        let mut raw = Commit::raw(public_key);
        {
            let mut payload = raw.payload_mut();
            LittleEndian::write_u64(&mut payload[0..8], height);
            &mut payload[8..40].copy_from_slice(hash.as_ref());
        }
        raw.sign(secret_key);
        Message::new(raw)
    }

    pub fn height(&self) -> u64 {
        LittleEndian::read_u64(&self.raw[0..8])
    }

    pub fn hash(&self) -> &Hash {
        unsafe {
            mem::transmute(&self.raw[8])
        }
    }
}

#[test]
fn test_connect() {
    use std::str::FromStr;

    let socket_address = net::SocketAddr::from_str("18.34.3.4:7777").unwrap();
    let time = ::time::get_time();
    let (public_key, secret_key) = super::crypto::gen_keypair();

    // write
    let message = Connect::new(&socket_address, time,
                               &public_key, &secret_key);
    // read
    let connect = Connect::from_raw(&message);

    assert_eq!(connect.socket_address(), socket_address);
    assert_eq!(connect.time(), time);
    assert!(message.verify());
}

#[test]
fn test_propose() {
    let height = 123_123_123;
    let round = 321_321_312;
    let time = ::time::get_time();
    let prev_hash = super::crypto::hash(&[1, 2, 3]);
    let (public_key, secret_key) = super::crypto::gen_keypair();

    // write
    let message = Propose::new(height, round, time, &prev_hash,
                               &public_key, &secret_key);
    // read
    let propose = Propose::from_raw(&message);

    assert_eq!(propose.height(), height);
    assert_eq!(propose.round(), round);
    assert_eq!(propose.time(), time);
    assert_eq!(propose.prev_hash(), &prev_hash);
    assert!(message.verify());
}

#[test]
fn test_prevote() {
    let height = 123_123_123;
    let round = 321_321_312;
    let hash = super::crypto::hash(&[1, 2, 3]);
    let (public_key, secret_key) = super::crypto::gen_keypair();

    // write
    let message = Prevote::new(height, round, &hash, &public_key, &secret_key);
    // read
    let prevote = Prevote::from_raw(&message);

    assert_eq!(prevote.height(), height);
    assert_eq!(prevote.round(), round);
    assert_eq!(prevote.hash(), &hash);
    assert!(message.verify());
}

#[test]
fn test_precommit() {
    let height = 123_123_123;
    let round = 321_321_312;
    let hash = super::crypto::hash(&[1, 2, 3]);
    let (public_key, secret_key) = super::crypto::gen_keypair();

    // write
    let message = Precommit::new(height, round, &hash,
                                 &public_key, &secret_key);
    // read
    let precommit = Precommit::from_raw(&message);

    assert_eq!(precommit.height(), height);
    assert_eq!(precommit.round(), round);
    assert_eq!(precommit.hash(), &hash);
    assert!(message.verify());
}

#[test]
fn test_commit() {
    let height = 123_123_123;
    let hash = super::crypto::hash(&[1, 2, 3]);
    let (public_key, secret_key) = super::crypto::gen_keypair();

    // write
    let message = Commit::new(height, &hash, &public_key, &secret_key);
    // read
    let commit = Commit::from_raw(&message);

    assert_eq!(commit.height(), height);
    assert_eq!(commit.hash(), &hash);
    assert!(message.verify());
}
