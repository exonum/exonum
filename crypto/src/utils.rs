// Copyright 2018 The Exonum Team
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

#![allow(dead_code)] // TODO Remove after complete ECR-2518 and  ECR-2519
use super::{gen_keypair, gen_keypair_from_seed, PublicKey, SecretKey, Seed, SEED_LENGTH};
use hex_buffer_serde::Hex;
use pwbox::{sodium::Sodium, ErasedPwBox, Eraser, Suite};
use rand::thread_rng;
use std::borrow::Cow;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{File, OpenOptions},
    io::{Error, ErrorKind, Read, Write},
    path::Path,
};
use toml;

/// Creates a TOML file that contains encrypted `SecretKey` and returns `PublicKey` for the secret key.
pub fn create_keys_file<P: AsRef<Path>>(path: P, pass_phrase: &[u8]) -> Result<PublicKey, Error> {
    let (pk, sk) = gen_keypair();
    let keys = EncryptedKeys::encrypt(pk, &sk, pass_phrase)?;
    let file_content =
        toml::to_string_pretty(&keys).map_err(|e| Error::new(ErrorKind::Other, e))?;
    let mut open_options = OpenOptions::new();
    open_options.create(true).write(true);
    #[cfg(unix)]
    open_options.mode(0o600);
    let mut file = open_options.open(path.as_ref())?;
    file.write_all(file_content.as_bytes())?;

    Ok(pk)
}

/// Reads `PublicKey` and `SecretKey` from encrypted file located by path and returns its.
pub fn read_keys_from_file<P: AsRef<Path>>(
    path: P,
    pass_phrase: &[u8],
) -> Result<(PublicKey, SecretKey), Error> {
    let mut key_file = File::open(path)?;

    if cfg!(unix) {
        let file_info = key_file.metadata()?;
        if (file_info.mode() & 0o600) != 0o600 {
            return Err(Error::new(ErrorKind::Other, "Wrong file's mode"));
        }
    }

    let mut file_content = vec![];
    key_file.read_to_end(&mut file_content)?;
    let keys: EncryptedKeys =
        toml::from_slice(file_content.as_slice()).map_err(|e| Error::new(ErrorKind::Other, e))?;
    keys.decrypt(pass_phrase)
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
        pass_phrase: &[u8],
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

    fn decrypt(self, pass_phrase: &[u8]) -> Result<(PublicKey, SecretKey), Error> {
        let mut eraser = Eraser::new();
        eraser.add_suite::<Sodium>();
        let restored = match eraser.restore(&self.secret_key) {
            Ok(restored) => restored,
            Err(_) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Couldn't restore a secret key",
                ))
            }
        };
        assert_eq!(restored.len(), SEED_LENGTH);
        let seed_bytes = restored
            .open(pass_phrase)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't open an encrypted key"))?
            .to_vec();
        let seed = Seed::from_slice(&seed_bytes[..])
            .ok_or_else(|| Error::new(ErrorKind::Other, "Couldn't create seed from slice"))?;
        let (public_key, secret_key) = gen_keypair_from_seed(&seed);
        assert_eq!(self.public_key, public_key);
        Ok((public_key, secret_key))
    }
}

#[cfg(test)]
mod tests {
    extern crate tempdir;

    use self::tempdir::TempDir;
    use super::*;
    use toml;

    #[test]
    fn test_create_and_read_keys_file() {
        let dir = TempDir::new("test_utils").expect("Couldn't create TempDir");
        let file_path = dir.path().join("private_key.toml");
        let pass_phrase = b"passphrase";
        let result = create_keys_file(file_path.as_path(), pass_phrase);
        assert!(result.is_ok());
        let pk1 = result.unwrap();
        let result = read_keys_from_file(file_path.as_path(), pass_phrase);
        assert!(result.is_ok());
        let (pk2, _) = result.unwrap();
        assert_eq!(pk1, pk2);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let pass_phrase = b"passphrase";
        let (pk, sk) = gen_keypair();

        let encrypt_key =
            EncryptedKeys::encrypt(pk, &sk, pass_phrase).expect("Couldn't encrypt keys");

        let (_, decrypted_key) = encrypt_key
            .decrypt(pass_phrase)
            .expect("Couldn't decrypt key");

        assert_eq!(sk, decrypted_key);
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

        let encrypt_key: EncryptedKeys =
            toml::from_str(file_content).expect("Couldn't deserialize content");
        let (public_key, decrypted_secret_key) = encrypt_key
            .decrypt(pass_phrase)
            .expect("Couldn't decrypt key");
        assert_eq!(
            public_key.to_hex(),
            "2e9d0b7ff996acdda58dd786950dec7361d3d81fd188cb250fd0cab2d064aaf8"
        );
        assert_eq!(decrypted_secret_key.to_hex(),
"47782139daefd1c1764d9ed0faa3e8e591c89a9c4e786758d196ed5041ca9e572e9d0b7ff996acdda58dd786950dec7361d3d81fd188cb250fd0cab2d064aaf8")
    }
}
