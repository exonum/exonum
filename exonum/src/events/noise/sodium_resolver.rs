use byteorder::{ByteOrder, LittleEndian};
use rand::{thread_rng, Rng};
use snow::params::{CipherChoice, DHChoice, HashChoice};
use snow::types::{Cipher, Dh, Hash, Random};
use snow::{CryptoResolver, DefaultResolver};

use sodiumoxide::crypto::aead::chacha20poly1305 as sodium_chacha20poly1305;
use sodiumoxide::crypto::hash::sha256 as sodium_sha256;

use crypto::x25519;
use crypto::{
    PUBLIC_KEY_LENGTH as SHA256_PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH as SHA256_SECRET_KEY_LENGTH,
};

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
    privkey: x25519::SecretKey,
    pubkey: x25519::PublicKey,
}

impl Default for SodiumDh25519 {
    fn default() -> SodiumDh25519 {
        SodiumDh25519 {
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
        &self.privkey.as_ref()
    }

    fn dh(&self, pubkey: &[u8], out: &mut [u8]) {
        assert_ne!(
            self.privkey,
            x25519::SecretKey::zero(),
            "Private key for SodiumDh25519 is not set."
        );

        let pubkey = x25519::PublicKey::from_slice(&pubkey[..x25519::PUBLIC_KEY_LENGTH])
            .expect("Can't construct public key for Dh25519");
        let result = x25519::scalarmult(&self.privkey, &pubkey);

        // FIXME: `snow` is able to pass incorrect public key, so this is a temporary workaround. (ECR-1726)
        if result.is_err() {
            error!("Can't calculate dh, public key {:?}", &pubkey[..]);
            return;
        }

        out[..self.pub_len()].copy_from_slice(&result.unwrap()[..self.pub_len()]);
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
        self.key = sodium_chacha20poly1305::Key::from_slice(&key[..32])
            .expect("Can't get key for ChaChaPoly");
    }

    fn encrypt(&self, nonce: u64, authtext: &[u8], plaintext: &[u8], out: &mut [u8]) -> usize {
        assert_ne!(
            self.key,
            SodiumChaChaPoly::default().key,
            "Can't encrypt with default key in SodiumChaChaPoly"
        );

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
        assert_ne!(
            self.key,
            SodiumChaChaPoly::default().key,
            "Can't dectypt with default key in SodiumChaChaPoly"
        );

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

    // Random data generator.
    struct MockRandom(u8);

    impl Default for MockRandom {
        fn default() -> MockRandom {
            MockRandom(0)
        }
    }

    impl Random for MockRandom {
        fn fill_bytes(&mut self, out: &mut [u8]) {
            let bytes = vec![self.0; out.len()];
            self.0 += 1;
            out.copy_from_slice(bytes.as_slice());
        }
    }

    #[test]
    fn test_curve25519() {
        // Values are cited from RFC-7748: 5.2.  Test Vectors.
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

    #[test]
    fn test_curve25519_shared_secret() {
        let mut rng = MockRandom::default();

        // Create two keypairs.
        let mut keypair_a = SodiumDh25519::default();
        keypair_a.generate(&mut rng);

        let mut keypair_b = SodiumDh25519::default();
        keypair_b.generate(&mut rng);

        // Create shared secrets with public keys of each other.
        let mut our_shared_secret = [0u8; 32];
        keypair_a.dh(keypair_b.pubkey(), &mut our_shared_secret);

        let mut remote_shared_secret = [0u8; 32];
        keypair_b.dh(keypair_a.pubkey(), &mut remote_shared_secret);

        // Results are expected to be the same.
        assert_eq!(our_shared_secret, remote_shared_secret);
    }
}
