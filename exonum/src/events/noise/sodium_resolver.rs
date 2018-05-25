// use byteorder::{ByteOrder, BigEndian, LittleEndian};
// use snow::constants::TAGLEN;
use snow::{CryptoResolver, DefaultResolver};
use snow::params::{DHChoice, HashChoice, CipherChoice};
use snow::types::{Random, Dh, Hash, Cipher};

use rand::{thread_rng, Rng};

// use sodiumoxide::crypto::onetimeauth::poly1305 as sodium_poly1305;
use sodiumoxide::crypto::scalarmult::curve25519 as sodium_curve25519;

// TODO REMOVE
#[allow(dead_code)]

pub struct SodiumResolver {
    parent: DefaultResolver,
}

impl SodiumResolver {
    pub fn new() -> Self {
        SodiumResolver { parent: DefaultResolver }
    }
}

impl CryptoResolver for SodiumResolver {
    fn resolve_rng(&self) -> Option<Box<Random + Send>> {
        Some(Box::new(SodiumRandom::default()))
    }

    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<Dh + Send>> {
        match *choice {
            DHChoice::Curve25519 => Some(Box::new(SodiumDh25519::default())),
            _                    => self.parent.resolve_dh(choice),
        }
    }

    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<Hash + Send>> {
        self.parent.resolve_hash(choice)
    }

    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<Cipher + Send>> {
        self.parent.resolve_cipher(choice)
    }
}

// Random data generator.
struct SodiumRandom;

impl Default for SodiumRandom {
    fn default() -> SodiumRandom {
        SodiumRandom {}
    }
}

impl Random for SodiumRandom {
    fn fill_bytes(&mut self, out: &mut [u8]) {
        thread_rng().fill(out); 
    }
}

// Elliptic curve 25519.
pub struct SodiumDh25519 {
    privkey: sodium_curve25519::Scalar,
    pubkey:  sodium_curve25519::GroupElement,
}

impl Default for SodiumDh25519 {
    fn default() -> SodiumDh25519 {
        SodiumDh25519 {
            privkey: sodium_curve25519::Scalar([0; 32]),
            pubkey: sodium_curve25519::GroupElement([0; 32]),
        }
    }
}

impl Dh for SodiumDh25519 {
    fn name(&self) -> &'static str {
        static NAME: &'static str = "25519";
        NAME
    }

    fn pub_len(&self) -> usize {
        32
    }

    fn priv_len(&self) -> usize {
        32
    }

    fn set(&mut self, privkey: &[u8]) {
        self.privkey = sodium_curve25519::Scalar::from_slice(privkey).expect("Can't construct private key for Dh25519");
        self.pubkey = sodium_curve25519::scalarmult_base(&self.privkey);
    }

    fn generate(&mut self, rng: &mut Random) {
        let mut privkey_bytes: [u8; 32];
        rng.fill_bytes(&mut privkey_bytes);
        privkey_bytes[0]  &= 248;
        privkey_bytes[31] &= 127;
        privkey_bytes[31] |= 64;
        self.privkey = sodium_curve25519::Scalar::from_slice(&privkey_bytes).expect("Can't construct private key for Dh25519");
        self.pubkey = sodium_curve25519::scalarmult_base(&self.privkey);
    }

    fn pubkey(&self) -> &[u8] {
        &self.pubkey[0..32]
    }

    fn privkey(&self) -> &[u8] {
        &self.privkey[0..32]
    }

    fn dh(&self, pubkey: &[u8], out: &mut [u8]) {
        let pubkey = sodium_curve25519::GroupElement::from_slice(pubkey).expect("Can't construct public key for Dh25519");
        let result = sodium_curve25519::scalarmult(&self.privkey, &pubkey).expect("Can't calculate dh");
        out.clone_from_slice(&result[0..32])
    }

}