use byteorder::{ByteOrder, LittleEndian};
use rand::{thread_rng, Rng};
use snow::params::{CipherChoice, DHChoice, HashChoice};
use snow::types::{Cipher, Dh, Hash, Random};
use snow::{CryptoResolver, DefaultResolver};

use sodiumoxide::crypto::aead::chacha20poly1305 as sodium_chacha20poly1305;
use sodiumoxide::crypto::hash::sha256 as sodium_sha256;
use sodiumoxide::crypto::scalarmult::curve25519 as sodium_curve25519;

pub struct SodiumResolver {
    parent: DefaultResolver,
}

impl SodiumResolver {
    pub fn new() -> Self {
        SodiumResolver {
            parent: DefaultResolver,
        }
    }
}

impl CryptoResolver for SodiumResolver {
    fn resolve_rng(&self) -> Option<Box<Random + Send>> {
        Some(Box::new(SodiumRandom::default()))
    }

    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<Dh + Send>> {
        match *choice {
            DHChoice::Curve25519 => Some(Box::new(SodiumDh25519::default())),
            _ => self.parent.resolve_dh(choice),
        }
    }

    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<Hash + Send>> {
        match *choice {
            HashChoice::SHA256 => Some(Box::new(SodiumSha256::default())),
            _ => self.parent.resolve_hash(choice),
        }
    }

    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<Cipher + Send>> {
        match *choice {
            CipherChoice::ChaChaPoly => Some(Box::new(SodiumChaChaPoly::default())),
            _ => self.parent.resolve_cipher(choice),
        }
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
        let bytes: Vec<u8> = thread_rng().gen_iter::<u8>().take(out.len()).collect();
        out.copy_from_slice(&bytes);
    }
}

// Elliptic curve 25519.
pub struct SodiumDh25519 {
    privkey: sodium_curve25519::Scalar,
    pubkey: sodium_curve25519::GroupElement,
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
        "25519"
    }

    fn pub_len(&self) -> usize {
        32
    }

    fn priv_len(&self) -> usize {
        32
    }

    fn set(&mut self, privkey: &[u8]) {
        self.privkey = sodium_curve25519::Scalar::from_slice(privkey)
            .expect("Can't construct private key for Dh25519");
        self.pubkey = sodium_curve25519::scalarmult_base(&self.privkey);
    }

    fn generate(&mut self, rng: &mut Random) {
        let mut privkey_bytes = [0; 32];
        rng.fill_bytes(&mut privkey_bytes);
        privkey_bytes[0] &= 248;
        privkey_bytes[31] &= 127;
        privkey_bytes[31] |= 64;
        self.privkey = sodium_curve25519::Scalar::from_slice(&privkey_bytes)
            .expect("Can't construct private key for Dh25519");
        self.pubkey = sodium_curve25519::scalarmult_base(&self.privkey);
    }

    fn pubkey(&self) -> &[u8] {
        &self.pubkey[0..32]
    }

    fn privkey(&self) -> &[u8] {
        &self.privkey[0..32]
    }

    fn dh(&self, pubkey: &[u8], out: &mut [u8]) {
        let pubkey = sodium_curve25519::GroupElement::from_slice(&pubkey[0..32])
            .expect("Can't construct public key for Dh25519");
        let result =
            sodium_curve25519::scalarmult(&self.privkey, &pubkey).expect("Can't calculate dh");

        out[..32].copy_from_slice(&result[0..32]);
    }
}

// Chacha20poly1305 cipher.

pub struct SodiumChaChaPoly {
    key: sodium_chacha20poly1305::Key,
}

impl Default for SodiumChaChaPoly {
    fn default() -> SodiumChaChaPoly {
        SodiumChaChaPoly {
            key: sodium_chacha20poly1305::Key([0; 32]),
        }
    }
}

impl Cipher for SodiumChaChaPoly {
    fn name(&self) -> &'static str {
        "ChaChaPoly"
    }

    fn set(&mut self, key: &[u8]) {
        self.key = sodium_chacha20poly1305::Key::from_slice(&key[0..32])
            .expect("Can't get key for ChaChaPoly");
    }

    fn encrypt(&self, nonce: u64, authtext: &[u8], plaintext: &[u8], out: &mut [u8]) -> usize {
        let mut nonce_bytes = [0u8; 8];
        LittleEndian::write_u64(&mut nonce_bytes[..], nonce);
        let nonce = sodium_chacha20poly1305::Nonce(nonce_bytes);

        let buf = sodium_chacha20poly1305::seal(plaintext, Some(authtext), &nonce, &self.key);

        out[..buf.len()].copy_from_slice(&buf);
        buf.len()
    }

    fn decrypt(
        &self,
        nonce: u64,
        authtext: &[u8],
        ciphertext: &[u8],
        out: &mut [u8],
    ) -> Result<usize, ()> {
        let mut nonce_bytes = [0u8; 8];
        LittleEndian::write_u64(&mut nonce_bytes[..], nonce);
        let nonce = sodium_chacha20poly1305::Nonce(nonce_bytes);

        let result = sodium_chacha20poly1305::open(ciphertext, Some(authtext), &nonce, &self.key);

        match result {
            Ok(ref buf) => {
                out[..buf.len()].copy_from_slice(&buf);
                Ok(buf.len())
            }
            Err(_) => Err(()),
        }
    }
}

// Hash Sha256.
#[derive(Debug, Default)]
struct SodiumSha256(sodium_sha256::State);

impl Hash for SodiumSha256 {
    fn name(&self) -> &'static str {
        "SHA256"
    }

    fn block_len(&self) -> usize {
        64
    }

    fn hash_len(&self) -> usize {
        32
    }

    fn reset(&mut self) {
        self.0 = sodium_sha256::State::init();
    }

    fn input(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    fn result(&mut self, out: &mut [u8]) {
        let digest = self.0.clone().finalize();
        out[..32].copy_from_slice(digest.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex::FromHex;

    #[test]
    fn test_curve25519() {
        // Curve25519 test - draft-curves-10
        let mut keypair: SodiumDh25519 = Default::default();
        let scalar = Vec::<u8>::from_hex(
            "a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4",
        ).unwrap();
        keypair.set(&scalar);
        let public = Vec::<u8>::from_hex(
            "e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c",
        ).unwrap();
        let mut output = [0u8; 32];
        keypair.dh(&public, &mut output);

        assert_eq!(
            output,
            Vec::<u8>::from_hex("c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552")
                .unwrap()
                .as_ref()
        );
    }
}
