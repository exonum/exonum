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

//! Key management for [Exonum] nodes.
//!
//! This crate provides tools for storing and loading encrypted keys for a node.
//!
//! [Exonum]: https://exonum.com/
//!
//! # Examples
//!
//! ```
//! use exonum_keys::{generate_keys, read_keys_from_file};
//! use tempdir::TempDir;
//!
//! # fn main() -> anyhow::Result<()> {
//! let dir = TempDir::new("test_keys")?;
//! let file_path = dir.path().join("private_key.toml");
//! let pass_phrase = b"super_secret_passphrase";
//! let keys = generate_keys(file_path.as_path(), pass_phrase)?;
//! let restored_keys = read_keys_from_file(file_path.as_path(), pass_phrase)?;
//! assert_eq!(keys, restored_keys);
//! # Ok(())
//! # }
//! ```

#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    // Next `cast_*` lints don't give alternatives.
    clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss,
    // Next lints produce too much noise/false positives.
    clippy::module_name_repetitions, clippy::similar_names, clippy::must_use_candidate,
    clippy::pub_enum_variant_names,
    // '... may panic' lints.
    clippy::indexing_slicing,
    // Too much work to fix.
    clippy::missing_errors_doc, clippy::missing_const_for_fn
)]

use anyhow::format_err;
use exonum_crypto::{KeyPair, PublicKey, SecretKey, Seed, SEED_LENGTH};
use pwbox::{sodium::Sodium, ErasedPwBox, Eraser, SensitiveData, Suite};
use rand::thread_rng;
use secret_tree::{Name, SecretTree};
use serde_derive::{Deserialize, Serialize};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{File, OpenOptions},
    io::{Error, ErrorKind, Read, Write},
    path::Path,
};

#[cfg(unix)]
#[cfg_attr(feature = "cargo-clippy", allow(clippy::verbose_bit_mask))]
fn validate_file_mode(mode: u32) -> Result<(), Error> {
    // Check that group and other bits are not set.
    if (mode & 0o_077) == 0 {
        Ok(())
    } else {
        Err(Error::new(ErrorKind::Other, "Wrong file's mode"))
    }
}

/// Container for all key pairs held by an Exonum node.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Keys {
    /// Consensus keypair.
    pub consensus: KeyPair,
    /// Service keypair.
    pub service: KeyPair,
}

impl Keys {
    /// Creates a random set of keys using the random number generator provided
    /// by the crypto backend.
    pub fn random() -> Self {
        Self {
            consensus: KeyPair::random(),
            service: KeyPair::random(),
        }
    }

    /// Creates validator keys from the provided keypairs.
    ///
    /// # Stability
    ///
    /// Since more keys may be added to `Keys` in the future, this method is considered
    /// unstable.
    ///
    /// # Panics
    ///
    /// If a public key in any keypair doesn't match with corresponding private key.
    pub fn from_keys(consensus_keys: impl Into<KeyPair>, service_keys: impl Into<KeyPair>) -> Self {
        Self {
            consensus: consensus_keys.into(),
            service: service_keys.into(),
        }
    }
}

impl Keys {
    /// Consensus public key.
    pub fn consensus_pk(&self) -> PublicKey {
        self.consensus.public_key()
    }

    /// Consensus private key.
    pub fn consensus_sk(&self) -> &SecretKey {
        self.consensus.secret_key()
    }

    /// Service public key.
    pub fn service_pk(&self) -> PublicKey {
        self.service.public_key()
    }

    /// Service secret key.
    pub fn service_sk(&self) -> &SecretKey {
        self.service.secret_key()
    }
}

fn save_master_key<P: AsRef<Path>>(
    path: P,
    encrypted_key: &EncryptedMasterKey,
) -> Result<(), Error> {
    let file_content =
        toml::to_string_pretty(encrypted_key).map_err(|e| Error::new(ErrorKind::Other, e))?;
    let mut open_options = OpenOptions::new();
    open_options.create(true).write(true);
    // By agreement we use the same permissions as for SSH private keys.
    #[cfg(unix)]
    open_options.mode(0o_600);
    let mut file = open_options.open(path.as_ref())?;
    file.write_all(file_content.as_bytes())?;

    Ok(())
}

/// Encrypted master key.
#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptedMasterKey {
    key: ErasedPwBox,
}

impl EncryptedMasterKey {
    fn encrypt(key: &secret_tree::Seed, pass_phrase: impl AsRef<[u8]>) -> Result<Self, Error> {
        let mut rng = thread_rng();
        let mut eraser = Eraser::new();
        eraser.add_suite::<Sodium>();
        let pwbox = Sodium::build_box(&mut rng)
            .seal(pass_phrase, key)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't create a pw box"))?;
        let encrypted_key = eraser
            .erase(&pwbox)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't convert a pw box"))?;

        Ok(Self { key: encrypted_key })
    }

    fn decrypt(self, pass_phrase: impl AsRef<[u8]>) -> Result<SensitiveData, Error> {
        let mut eraser = Eraser::new();
        eraser.add_suite::<Sodium>();
        let restored = eraser
            .restore(&self.key)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't restore a secret key"))?;
        assert_eq!(restored.len(), SEED_LENGTH);

        restored
            .open(pass_phrase)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't open an encrypted key"))
    }
}

/// Creates a TOML file that contains encrypted master and returns `Keys` derived from it.
pub fn generate_keys<P: AsRef<Path>>(path: P, passphrase: &[u8]) -> anyhow::Result<Keys> {
    let tree = SecretTree::new(&mut thread_rng());
    let encrypted_key = EncryptedMasterKey::encrypt(tree.seed(), passphrase)?;
    save_master_key(path, &encrypted_key)?;

    Ok(generate_keys_from_master_password(&tree))
}

/// Creates a TOML file from seed that contains encrypted master and returns `Keys` derived from it.
pub fn generate_keys_from_seed(
    passphrase: &[u8],
    seed: &[u8],
) -> anyhow::Result<(Keys, EncryptedMasterKey)> {
    let tree = SecretTree::from_seed(seed)
        .ok_or_else(|| format_err!("Error creating SecretTree from seed"))?;
    let encrypted_key = EncryptedMasterKey::encrypt(tree.seed(), passphrase)?;
    let keys = generate_keys_from_master_password(&tree);

    Ok((keys, encrypted_key))
}

fn generate_keys_from_master_password(tree: &SecretTree) -> Keys {
    let mut buffer = [0_u8; 32];

    tree.child(Name::new("consensus")).fill(&mut buffer);
    let seed = Seed::new(buffer);
    let consensus_keys = KeyPair::from_seed(&seed);

    tree.child(Name::new("service")).fill(&mut buffer);
    let seed = Seed::new(buffer);
    let service_keys = KeyPair::from_seed(&seed);

    Keys::from_keys(consensus_keys, service_keys)
}

/// Reads encrypted master key from file and generate validator keys from it.
pub fn read_keys_from_file<P: AsRef<Path>, W: AsRef<[u8]>>(
    path: P,
    pass_phrase: W,
) -> anyhow::Result<Keys> {
    let mut key_file = File::open(path)?;

    #[cfg(unix)]
    validate_file_mode(key_file.metadata()?.mode())?;

    let mut file_content = vec![];
    key_file.read_to_end(&mut file_content)?;
    let keys: EncryptedMasterKey =
        toml::from_slice(file_content.as_slice()).map_err(|e| Error::new(ErrorKind::Other, e))?;
    let seed = keys.decrypt(pass_phrase)?;
    let tree = SecretTree::from_seed(&seed).expect("Error creating secret tree from seed.");

    Ok(generate_keys_from_master_password(&tree))
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
        let pk1 = generate_keys(file_path.as_path(), pass_phrase).unwrap();
        let pk2 = read_keys_from_file(file_path.as_path(), pass_phrase).unwrap();
        assert_eq!(pk1, pk2);
    }

    #[test]
    fn encrypt_decrypt() {
        let pass_phrase = b"passphrase";
        let tree = SecretTree::new(&mut thread_rng());
        let seed = tree.seed();
        let key =
            EncryptedMasterKey::encrypt(seed, pass_phrase).expect("Couldn't encrypt master key");

        let decrypted_seed = key
            .decrypt(pass_phrase)
            .expect("Couldn't decrypt master key ");
        assert_eq!(&seed[..], &decrypted_seed[..]);
    }

    #[test]
    fn test_decrypt_from_file() {
        let pass_phrase = b"passphrase";
        let file_content = r#"
          [key]
          ciphertext = "cf6c63520e789efc978ad07e218e6fd199ccb5e861e9c893cac40641fc66c89c"
          mac = "b3e1a815a2cb316bf209da7bc4203091"
          kdf = "scrypt-nacl"
          cipher = "xsalsa20-poly1305"

          [key.kdfparams]
          salt = "7e7a1d9dc5269b0ebedb5fcd433d772880786ebefe8647a275ef1626cfb122b3"
          memlimit = 16777216
          opslimit = 524288

          [key.cipherparams]
          iv = "832bded4e12948a022065ace39b31cd7b514bf9c2b2a407f"
        "#;

        let keys: EncryptedMasterKey =
            toml::from_str(file_content).expect("Couldn't deserialize content");
        let seed = keys.decrypt(pass_phrase).expect("Couldn't decrypt key");

        assert_eq!(
            hex::encode(&*seed),
            "a05a82575d5f9d1f9469df31896f5b3c14ec4d18b3948cd7c8b09a7eed48b4e0"
        );
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
