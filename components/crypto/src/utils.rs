// Copyright 2019 The Exonum Team
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

// spell-checker:ignore cipherparams ciphertext

use super::{gen_keypair, gen_keypair_from_seed, kx, PublicKey, SecretKey, Seed, SEED_LENGTH};
use exonum_sodiumoxide::crypto::pwhash::{gen_salt, Salt};
use hex_buffer_serde::Hex;
use pwbox::{sodium::Sodium, ErasedPwBox, Eraser, Suite};
use rand::thread_rng;
use secret_tree::{Name, SecretTree};
use std::borrow::Cow;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{File, OpenOptions},
    io::{Error, ErrorKind, Read, Write},
    path::Path,
};

/// Creates a TOML file that contains encrypted `SecretKey` and returns `PublicKey` for the secret key.
pub fn generate_keys_file<P: AsRef<Path>, W: AsRef<[u8]>>(
    path: P,
    pass_phrase: W,
) -> Result<PublicKey, Error> {
    let (pk, sk) = gen_keypair();
    let keys = EncryptedKeys::encrypt(pk, &sk, pass_phrase)?;
    let file_content =
        toml::to_string_pretty(&keys).map_err(|e| Error::new(ErrorKind::Other, e))?;
    let mut open_options = OpenOptions::new();
    open_options.create(true).write(true);
    #[cfg(unix)]
    open_options.mode(0o_600);
    let mut file = open_options.open(path.as_ref())?;
    file.write_all(file_content.as_bytes())?;

    Ok(pk)
}

///// Reads and returns `PublicKey` and `SecretKey` from encrypted file located by path and returns its.
//pub fn read_keys_from_file<P: AsRef<Path>, W: AsRef<[u8]>>(
//    path: P,
//    pass_phrase: W,
//) -> Result<(PublicKey, SecretKey), Error> {
//    let mut key_file = File::open(path)?;
//
//    #[cfg(unix)]
//    validate_file_mode(key_file.metadata()?.mode())?;
//
//    let mut file_content = vec![];
//    key_file.read_to_end(&mut file_content)?;
//    let keys: EncryptedKeys =
//        toml::from_slice(file_content.as_slice()).map_err(|e| Error::new(ErrorKind::Other, e))?;
//    keys.decrypt(pass_phrase)
//}

#[cfg(unix)]
#[cfg_attr(feature = "cargo-clippy", allow(clippy::verbose_bit_mask))]
fn validate_file_mode(mode: u32) -> Result<(), Error> {
    if (mode & 0o_077) == 0 {
        Ok(())
    } else {
        Err(Error::new(ErrorKind::Other, "Wrong file's mode"))
    }
}

struct PublicKeyHex;

impl Hex<PublicKey> for PublicKeyHex {
    fn create_bytes(value: &PublicKey) -> Cow<[u8]> {
        Cow::Borrowed(&*value.as_ref())
    }

    fn from_bytes(bytes: &[u8]) -> Result<PublicKey, String> {
        PublicKey::from_slice(bytes)
            .ok_or_else(|| "Couldn't create PublicKey from slice".to_owned())
    }
}

#[derive(Serialize, Deserialize)]
struct EncryptedKeys {
    #[serde(with = "PublicKeyHex")]
    public_key: PublicKey,
    secret_key: ErasedPwBox,
}

impl EncryptedKeys {
    fn encrypt(
        public_key: PublicKey,
        secret_key: &SecretKey,
        pass_phrase: impl AsRef<[u8]>,
    ) -> Result<EncryptedKeys, Error> {
        let mut rng = thread_rng();
        let mut eraser = Eraser::new();
        let seed = &secret_key[..SEED_LENGTH];
        eraser.add_suite::<Sodium>();
        let pwbox = Sodium::build_box(&mut rng)
            .seal(pass_phrase, &seed)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't create a pw box"))?;
        let encrypted_key = eraser
            .erase(&pwbox)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't convert a pw box"))?;

        Ok(EncryptedKeys {
            public_key,
            secret_key: encrypted_key,
        })
    }

    fn decrypt(self, pass_phrase: impl AsRef<[u8]>) -> Result<(PublicKey, SecretKey), Error> {
        let mut eraser = Eraser::new();
        eraser.add_suite::<Sodium>();
        let restored = eraser
            .restore(&self.secret_key)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't restore a secret key"))?;
        assert_eq!(restored.len(), SEED_LENGTH);
        let seed_bytes = restored
            .open(pass_phrase)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't open an encrypted key"))?;
        let seed = Seed::from_slice(&seed_bytes[..])
            .ok_or_else(|| Error::new(ErrorKind::Other, "Couldn't create seed from slice"))?;
        let (public_key, secret_key) = gen_keypair_from_seed(&seed);

        if self.public_key == public_key {
            Ok((public_key, secret_key))
        } else {
            Err(Error::new(ErrorKind::Other, "Different public keys"))
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Keys {
    pub consensus_pk: PublicKey,
    pub consensus_sk: SecretKey,
    pub service_pk: PublicKey,
    pub service_sk: SecretKey,
    pub identity_pk: kx::PublicKey,
    pub identity_sk: kx::SecretKey,
}

pub fn save_master_key<P: AsRef<Path>, W: AsRef<[u8]>>(
    path: P,
    pass_phrase: W,
    key: Salt,
) -> Result<(), Error> {
    let encrypted_key = EncryptedMasterKey::encrypt(key, pass_phrase)?;
    let file_content =
        toml::to_string_pretty(&encrypted_key).map_err(|e| Error::new(ErrorKind::Other, e))?;
    let mut open_options = OpenOptions::new();
    open_options.create(true).write(true);
    #[cfg(unix)]
    open_options.mode(0o_600);
    let mut file = open_options.open(path.as_ref())?;
    file.write_all(file_content.as_bytes())?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct EncryptedMasterKey {
    key: ErasedPwBox,
}

impl EncryptedMasterKey {
    fn encrypt(key: Salt, pass_phrase: impl AsRef<[u8]>) -> Result<EncryptedMasterKey, Error> {
        let mut rng = thread_rng();
        let mut eraser = Eraser::new();
        eraser.add_suite::<Sodium>();
        let pwbox = Sodium::build_box(&mut rng)
            .seal(pass_phrase, key)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't create a pw box"))?;
        let encrypted_key = eraser
            .erase(&pwbox)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't convert a pw box"))?;

        Ok(EncryptedMasterKey { key: encrypted_key })
    }

    fn decrypt(self, pass_phrase: impl AsRef<[u8]>) -> Result<Salt, Error> {
        let mut eraser = Eraser::new();
        eraser.add_suite::<Sodium>();
        let restored = eraser
            .restore(&self.key)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't restore a secret key"))?;
        assert_eq!(restored.len(), SEED_LENGTH);
        let seed_bytes = restored
            .open(pass_phrase)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't open an encrypted key"))?;
        let salt = Salt::from_slice(&seed_bytes[..])
            .ok_or_else(|| Error::new(ErrorKind::Other, "Couldn't create seed from slice"))?;

        Ok(salt)
    }
}

pub fn generate_keys<P: AsRef<Path>>(path: P, passphrase: &[u8]) -> Keys {
    let salt = gen_salt();
    save_master_key(path, passphrase, salt).expect("Error generating master key.");
    generate_keys_from_master_password(salt)
}

fn generate_keys_from_master_password(salt: Salt) -> Keys {
    let tree = SecretTree::from_seed(salt.as_ref()).unwrap();
    let mut buffer = [0_u8; 32];

    tree.child(Name::new("consensus")).fill(&mut buffer);
    let seed = Seed::from_slice(&buffer).unwrap();
    let (consensus_pk, consensus_sk) = gen_keypair_from_seed(&seed);

    tree.child(Name::new("service")).fill(&mut buffer);
    let seed = Seed::from_slice(&buffer).unwrap();
    let (service_pk, service_sk) = gen_keypair_from_seed(&seed);

    tree.child(Name::new("identity")).fill(&mut buffer);
    let seed = Seed::from_slice(&buffer).unwrap();
    let (identity_pk, identity_sk) = kx::gen_keypair_from_seed(&seed);

    Keys {
        consensus_pk,
        consensus_sk,
        service_pk,
        service_sk,
        identity_pk,
        identity_sk,
    }
}

pub fn read_keys_from_file<P: AsRef<Path>, W: AsRef<[u8]>>(
    path: P,
    pass_phrase: W,
) -> Result<Keys, Error> {
    let mut key_file = File::open(path)?;

    #[cfg(unix)]
    validate_file_mode(key_file.metadata()?.mode())?;

    let mut file_content = vec![];
    key_file.read_to_end(&mut file_content)?;
    let keys: EncryptedMasterKey =
        toml::from_slice(file_content.as_slice()).map_err(|e| Error::new(ErrorKind::Other, e))?;
    let salt = keys.decrypt(pass_phrase)?;

    Ok(generate_keys_from_master_password(salt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    #[test]
    fn test_create_and_read_keys_file() {
        let dir = TempDir::new("test_utils").expect("Couldn't create TempDir");
        let file_path = dir.path().join("private_key.toml");
        let pass_phrase = b"passphrase";
        let pk1 = generate_keys_file(file_path.as_path(), pass_phrase).unwrap();
        let (pk2, _) = read_keys_from_file(file_path.as_path(), pass_phrase).unwrap();
        assert_eq!(pk1, pk2);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let pass_phrase = b"passphrase";
        let (pk, sk) = gen_keypair();
        let keys = EncryptedKeys::encrypt(pk, &sk, pass_phrase).expect("Couldn't encrypt keys");
        let (_, decrypted_sk) = keys.decrypt(pass_phrase).expect("Couldn't decrypt key");
        assert_eq!(sk, decrypted_sk);
    }

    #[test]
    fn test_decrypt_from_file() {
        let pass_phrase = b"passphrase";
        let file_content = r#"
            public_key = '2e9d0b7ff996acdda58dd786950dec7361d3d81fd188cb250fd0cab2d064aaf8'

            [secret_key]
            ciphertext = '7fbb51090742482da42816b2c908ff61c470a19ca1b984014c7ac37dd46ef1ef'
            mac = '862f27c67b07f9665628b6f9a72a1c20'
            kdf = 'scrypt-nacl'
            cipher = 'xsalsa20-poly1305'

            [secret_key.kdfparams]
            salt = '2ee70102a15aff032523a5df91e435172ef003ad9898a3a5eb2f5af447d28b63'
            memlimit = 16777216
            opslimit = 524288

            [secret_key.cipherparams]
            iv = '374c8dc0ab8d753ae0515f485e24f6c76b469cde3dee285c'
        "#;

        let keys: EncryptedKeys =
            toml::from_str(file_content).expect("Couldn't deserialize content");
        let (public_key, secret_key) = keys.decrypt(pass_phrase).expect("Couldn't decrypt key");
        assert_eq!(
            public_key.to_hex(),
            "2e9d0b7ff996acdda58dd786950dec7361d3d81fd188cb250fd0cab2d064aaf8"
        );
        assert_eq!(
            secret_key.to_hex(),
            "47782139daefd1c1764d9ed0faa3e8e591c89a9c4e786758d196ed5041ca9e57\
             2e9d0b7ff996acdda58dd786950dec7361d3d81fd188cb250fd0cab2d064aaf8"
        )
    }

    #[cfg(unix)]
    #[test]
    fn test_validate_file_mode() {
        assert!(validate_file_mode(0o_100_600).is_ok());
        assert!(validate_file_mode(0o_600).is_ok());
        assert!(validate_file_mode(0o_111_111).is_err());
        assert!(validate_file_mode(0o_100_644).is_err());
        assert!(validate_file_mode(0o_100_666).is_err());
        assert!(validate_file_mode(0o_100_777).is_err());
        assert!(validate_file_mode(0o_100_755).is_err());
        assert!(validate_file_mode(0o_644).is_err());
        assert!(validate_file_mode(0o_666).is_err());
    }
}
