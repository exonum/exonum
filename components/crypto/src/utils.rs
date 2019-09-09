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

use super::{
    gen_keypair_from_seed, kx, PublicKey, SecretKey, Seed, PUBLIC_KEY_LENGTH, SEED_LENGTH,
};
use failure::format_err;
use pwbox::{sodium::Sodium, ErasedPwBox, Eraser, Suite};
use rand::thread_rng;
use secret_tree::{Name, SecretTree};

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
    if (mode & 0o_077) == 0 {
        Ok(())
    } else {
        Err(Error::new(ErrorKind::Other, "Wrong file's mode"))
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyPair {
    public_key: PublicKey,
    secret_key: SecretKey,
}

impl KeyPair {
    fn from_keys(public_key: PublicKey, secret_key: SecretKey) -> Self {
        assert_eq!(
            &public_key[..],
            &secret_key[PUBLIC_KEY_LENGTH..],
            "Public key does not match the secret key."
        );

        Self {
            public_key,
            secret_key,
        }
    }
}

/// Struct containing all validator key pairs.
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Keys {
    consensus: KeyPair,
    service: KeyPair,
    identity: kx::KeyPair,
}

impl Keys {
    pub fn from_keys(
        consensus_pk: PublicKey,
        consensus_sk: SecretKey,
        service_pk: PublicKey,
        service_sk: SecretKey,
        identity_pk: kx::PublicKey,
        identity_sk: kx::SecretKey,
    ) -> Self {
        Self {
            consensus: KeyPair::from_keys(consensus_pk, consensus_sk),
            service: KeyPair::from_keys(service_pk, service_sk),
            identity: kx::KeyPair::from_keys(identity_pk, identity_sk),
        }
    }
}

impl Keys {
    pub fn consensus_pk(&self) -> PublicKey {
        self.consensus.public_key
    }

    pub fn consensus_sk(&self) -> &SecretKey {
        &self.consensus.secret_key
    }

    pub fn service_pk(&self) -> PublicKey {
        self.service.public_key
    }

    pub fn service_sk(&self) -> &SecretKey {
        &self.service.secret_key
    }

    pub fn identity_pk(&self) -> kx::PublicKey {
        self.identity.public_key
    }

    pub fn identity_sk(&self) -> &kx::SecretKey {
        &self.identity.secret_key
    }
}

pub fn save_master_key<P: AsRef<Path>, W: AsRef<[u8]>>(
    path: P,
    pass_phrase: W,
    key: &secret_tree::Seed,
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
    fn encrypt(
        key: &secret_tree::Seed,
        pass_phrase: impl AsRef<[u8]>,
    ) -> Result<EncryptedMasterKey, Error> {
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

    fn decrypt(self, pass_phrase: impl AsRef<[u8]>) -> Result<secret_tree::Seed, Error> {
        let mut eraser = Eraser::new();
        eraser.add_suite::<Sodium>();
        let restored = eraser
            .restore(&self.key)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't restore a secret key"))?;
        assert_eq!(restored.len(), SEED_LENGTH);
        let seed_bytes = restored
            .open(pass_phrase)
            .map_err(|_| Error::new(ErrorKind::Other, "Couldn't open an encrypted key"))?;

        let mut seed: [u8; 32] = [0; 32];
        seed.copy_from_slice(&seed_bytes[..]);
        Ok(seed)
    }
}

/// Creates a TOML file that contains encrypted master and returns `Keys` derived from it.
pub fn generate_keys<P: AsRef<Path>>(path: P, passphrase: &[u8]) -> Keys {
    let tree = SecretTree::new(&mut thread_rng());
    save_master_key(path, passphrase, tree.seed()).expect("Error generating master key.");
    generate_keys_from_master_password(tree).expect("Error deriving keys from master key.")
}

fn generate_keys_from_master_password(tree: SecretTree) -> Option<Keys> {
    let mut buffer = [0_u8; 32];

    tree.child(Name::new("consensus")).fill(&mut buffer);
    let seed = Seed::from_slice(&buffer)?;
    let (consensus_pk, consensus_sk) = gen_keypair_from_seed(&seed);

    tree.child(Name::new("service")).fill(&mut buffer);
    let seed = Seed::from_slice(&buffer)?;
    let (service_pk, service_sk) = gen_keypair_from_seed(&seed);

    tree.child(Name::new("identity")).fill(&mut buffer);
    let seed = Seed::from_slice(&buffer)?;
    let (identity_pk, identity_sk) = kx::gen_keypair_from_seed(&seed);

    Some(Keys::from_keys(
        consensus_pk,
        consensus_sk,
        service_pk,
        service_sk,
        identity_pk,
        identity_sk,
    ))
}

pub fn read_keys_from_file<P: AsRef<Path>, W: AsRef<[u8]>>(
    path: P,
    pass_phrase: W,
) -> Result<Keys, failure::Error> {
    let mut key_file = File::open(path)?;

    #[cfg(unix)]
    validate_file_mode(key_file.metadata()?.mode())?;

    let mut file_content = vec![];
    key_file.read_to_end(&mut file_content)?;
    let keys: EncryptedMasterKey =
        toml::from_slice(file_content.as_slice()).map_err(|e| Error::new(ErrorKind::Other, e))?;
    let seed = keys.decrypt(pass_phrase)?;

    let tree = SecretTree::from_seed(&seed).expect("Error creating secret tree from seed.");
    generate_keys_from_master_password(tree)
        .ok_or_else(|| format_err!("Error deriving keys from master key"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_keypair;
    use tempdir::TempDir;

    #[test]
    fn test_create_and_read_keys_file() {
        let dir = TempDir::new("test_utils").expect("Couldn't create TempDir");
        let file_path = dir.path().join("private_key.toml");
        let pass_phrase = b"passphrase";
        let pk1 = generate_keys(file_path.as_path(), pass_phrase);
        let pk2 = read_keys_from_file(file_path.as_path(), pass_phrase).unwrap();
        assert_eq!(pk1, pk2);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let pass_phrase = b"passphrase";
        let tree = SecretTree::new(&mut thread_rng());
        let seed = tree.seed();
        let key =
            EncryptedMasterKey::encrypt(&seed, pass_phrase).expect("Couldn't encrypt master key");

        dbg!(hex::encode(&seed));

        let mut file = File::create("foo.txt").unwrap();
        let _ = file.write_all(toml::to_string(&key).unwrap().as_bytes());

        let decrypted_seed = key
            .decrypt(pass_phrase)
            .expect("Couldn't decrypt master key ");
        assert_eq!(seed, &decrypted_seed);
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
            hex::encode(&seed),
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

    #[test]
    fn valid_keypair() {
        let (pk, sk) = gen_keypair();
        let _ = KeyPair::from_keys(pk, sk);
    }

    #[test]
    #[should_panic]
    fn not_valid_keypair() {
        let (pk, _) = gen_keypair();
        let (_, sk) = gen_keypair();
        let _ = KeyPair::from_keys(pk, sk);
    }

    #[test]
    fn valid_kx_keypair() {
        let (pk, sk) = kx::gen_keypair();
        let _ = kx::KeyPair::from_keys(pk, sk);
    }

    #[test]
    #[should_panic]
    fn not_valid_kx_keypair() {
        let (pk, _) = kx::gen_keypair();
        let (_, sk) = kx::gen_keypair();
        let _ = kx::KeyPair::from_keys(pk, sk);
    }
}
