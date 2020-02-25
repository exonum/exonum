// Copyright 2020 The Exonum Team
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

// spell-checker:ignore chacha, privkey, authtext, ciphertext

use byteorder::{ByteOrder, LittleEndian};
use exonum_sodiumoxide::crypto::{
    aead::chacha20poly1305_ietf as sodium_chacha20poly1305, hash::sha256 as sodium_sha256,
};
use log::error;
use rand::{thread_rng, CryptoRng, Error, RngCore};
use snow::{
    params::{CipherChoice, DHChoice, HashChoice},
    resolvers::CryptoResolver,
    types::{Cipher, Dh, Hash, Random},
};

use crate::crypto::{
    x25519, PUBLIC_KEY_LENGTH as SHA256_PUBLIC_KEY_LENGTH,
    SECRET_KEY_LENGTH as SHA256_SECRET_KEY_LENGTH,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SodiumResolver;

impl SodiumResolver {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CryptoResolver for SodiumResolver {
    fn resolve_rng(&self) -> Option<Box<dyn Random>> {
        Some(Box::new(SodiumRandom::default()))
    }

    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<dyn Dh>> {
        match *choice {
            DHChoice::Curve25519 => Some(Box::new(SodiumDh25519::default())),
            _ => None,
        }
    }

    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<dyn Hash>> {
        match *choice {
            HashChoice::SHA256 => Some(Box::new(SodiumSha256::default())),
            _ => None,
        }
    }

    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<dyn Cipher>> {
        match *choice {
            CipherChoice::ChaChaPoly => Some(Box::new(SodiumChaChaPoly::default())),
            _ => None,
        }
    }
}

// Random data generator.
struct SodiumRandom;

impl Default for SodiumRandom {
    fn default() -> Self {
        Self {}
    }
}

impl Random for SodiumRandom {}

impl CryptoRng for SodiumRandom {}

impl RngCore for SodiumRandom {
    fn next_u32(&mut self) -> u32 {
        unreachable!()
    }

    fn next_u64(&mut self) -> u64 {
        unreachable!()
    }

    fn fill_bytes(&mut self, out: &mut [u8]) {
        thread_rng().fill_bytes(out);
    }

    fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), Error> {
        unreachable!()
    }
}

// Elliptic curve 25519.
pub struct SodiumDh25519 {
    privkey: x25519::SecretKey,
    pubkey: x25519::PublicKey,
}

impl Default for SodiumDh25519 {
    fn default() -> Self {
        Self {
            privkey: x25519::SecretKey::zero(),
            pubkey: x25519::PublicKey::zero(),
        }
    }
}

impl Dh for SodiumDh25519 {
    fn name(&self) -> &'static str {
        "25519"
    }

    fn pub_len(&self) -> usize {
        x25519::PUBLIC_KEY_LENGTH
    }

    fn priv_len(&self) -> usize {
        x25519::SECRET_KEY_LENGTH
    }

    fn set(&mut self, privkey: &[u8]) {
        self.privkey = x25519::SecretKey::from_slice(privkey)
            .expect("Can't construct private key for Dh25519");
        self.pubkey = x25519::scalarmult_base(&self.privkey);
    }

    fn generate(&mut self, rng: &mut dyn Random) {
        let mut privkey_bytes = [0; x25519::SECRET_KEY_LENGTH];
        rng.fill_bytes(&mut privkey_bytes);
        x25519::convert_to_private_key(&mut privkey_bytes);
        self.set(&privkey_bytes);
    }

    fn pubkey(&self) -> &[u8] {
        self.pubkey.as_ref()
    }

    fn privkey(&self) -> &[u8] {
        self.privkey.as_ref()
    }

    fn dh(&self, pubkey: &[u8], out: &mut [u8]) -> Result<(), ()> {
        assert_ne!(
            self.privkey,
            x25519::SecretKey::zero(),
            "Private key for SodiumDh25519 is not set."
        );

        let pubkey = x25519::PublicKey::from_slice(&pubkey[..x25519::PUBLIC_KEY_LENGTH])
            .expect("Can't construct public key for Dh25519");
        let result = x25519::scalarmult(&self.privkey, &pubkey);

        if result.is_err() {
            error!("Can't calculate dh, public key {:?}", &pubkey[..]);
            return Err(());
        }

        out[..self.pub_len()].copy_from_slice(&result.unwrap()[..self.pub_len()]);
        Ok(())
    }
}

// Chacha20poly1305 cipher.
pub struct SodiumChaChaPoly {
    key: sodium_chacha20poly1305::Key,
}

impl SodiumChaChaPoly {
    // In IETF version of `chacha20poly1305` nonce has 12 bytes instead of 8.
    fn get_ietf_nonce(nonce: u64) -> sodium_chacha20poly1305::Nonce {
        let mut nonce_bytes = [0_u8; 12];
        LittleEndian::write_u64(&mut nonce_bytes[4..], nonce);
        sodium_chacha20poly1305::Nonce(nonce_bytes)
    }
}

impl Default for SodiumChaChaPoly {
    fn default() -> Self {
        Self {
            key: sodium_chacha20poly1305::Key([0; 32]),
        }
    }
}

impl Cipher for SodiumChaChaPoly {
    fn name(&self) -> &'static str {
        "ChaChaPoly"
    }

    fn set(&mut self, key: &[u8]) {
        self.key = sodium_chacha20poly1305::Key::from_slice(&key[..32])
            .expect("Can't get key for ChaChaPoly");
    }

    fn encrypt(&self, nonce: u64, authtext: &[u8], plaintext: &[u8], out: &mut [u8]) -> usize {
        assert_ne!(
            self.key,
            Self::default().key,
            "Can't encrypt with default key in SodiumChaChaPoly"
        );

        let nonce = Self::get_ietf_nonce(nonce);

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
        assert_ne!(
            self.key,
            Self::default().key,
            "Can't decrypt with default key in SodiumChaChaPoly"
        );

        let nonce = Self::get_ietf_nonce(nonce);

        let result = sodium_chacha20poly1305::open(ciphertext, Some(authtext), &nonce, &self.key);

        match result {
            Ok(ref buf) => {
                out[..buf.len()].copy_from_slice(buf);
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
        SHA256_SECRET_KEY_LENGTH
    }

    fn hash_len(&self) -> usize {
        SHA256_PUBLIC_KEY_LENGTH
    }

    fn reset(&mut self) {
        self.0 = sodium_sha256::State::init();
    }

    fn input(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    fn result(&mut self, out: &mut [u8]) {
        let digest = self.0.clone().finalize();
        out[..self.hash_len()].copy_from_slice(digest.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex::FromHex;
    use pretty_assertions::assert_eq;

    // Random data generator.
    struct MockRandom(u8);

    impl Random for MockRandom {}

    impl CryptoRng for MockRandom {}

    impl RngCore for MockRandom {
        fn next_u32(&mut self) -> u32 {
            unreachable!()
        }

        fn next_u64(&mut self) -> u64 {
            unreachable!()
        }

        fn fill_bytes(&mut self, out: &mut [u8]) {
            let bytes = vec![self.0; out.len()];
            self.0 += 1;
            out.copy_from_slice(bytes.as_slice());
        }

        fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), Error> {
            unreachable!()
        }
    }

    impl Default for MockRandom {
        fn default() -> Self {
            Self(0)
        }
    }

    #[test]
    fn test_curve25519() {
        // Values are cited from RFC-7748: 5.2.  Test Vectors.
        let mut keypair = SodiumDh25519::default();
        let scalar =
            Vec::<u8>::from_hex("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4")
                .unwrap();
        keypair.set(&scalar);
        let public =
            Vec::<u8>::from_hex("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c")
                .unwrap();
        let mut output = [0_u8; 32];
        keypair.dh(&public, &mut output).unwrap();

        assert_eq!(
            output,
            Vec::<u8>::from_hex("c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552")
                .unwrap()
                .as_ref()
        );
    }

    #[test]
    fn test_curve25519_shared_secret() {
        let mut rng = MockRandom::default();

        // Create two keypairs.
        let mut keypair_a = SodiumDh25519::default();
        keypair_a.generate(&mut rng);

        let mut keypair_b = SodiumDh25519::default();
        keypair_b.generate(&mut rng);

        // Create shared secrets with public keys of each other.
        let mut our_shared_secret = [0_u8; 32];
        keypair_a
            .dh(keypair_b.pubkey(), &mut our_shared_secret)
            .unwrap();

        let mut remote_shared_secret = [0_u8; 32];
        keypair_b
            .dh(keypair_a.pubkey(), &mut remote_shared_secret)
            .unwrap();

        // Results are expected to be the same.
        assert_eq!(our_shared_secret, remote_shared_secret);
    }
}
