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
use super::{gen_keypair, PublicKey, SecretKey};
use pwbox::{sodium::Sodium, ErasedPwBox, Eraser, Suite};
use rand::thread_rng;
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
    let sk_bytes = hex::decode(sk.to_hex()).map_err(|e| Error::new(ErrorKind::Other, e))?;
    let keys = encrypt(pk.to_hex(), &sk_bytes, pass_phrase)?;
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
    let pub_key = PublicKey::from_slice(
        &hex::decode(&keys.public_key).map_err(|e| Error::new(ErrorKind::Other, e))?,
    ).ok_or_else(|| Error::new(
        ErrorKind::Other,
        "Couldn't create PublicKey from slice",
    ))?;
    let sec_key = SecretKey::from_slice(&decrypt(&keys, pass_phrase)?).ok_or_else(|| Error::new(
        ErrorKind::Other,
        "Couldn't create SecretKey from slice",
    ))?;

    Ok((pub_key, sec_key))
}

#[derive(Serialize, Deserialize)]
struct EncryptedKeys {
    public_key: String,
    secret_key: ErasedPwBox,
}

fn encrypt(
    public_key: String,
    secret_key: &[u8],
    pass_phrase: &[u8],
) -> Result<EncryptedKeys, Error> {
    let mut rng = thread_rng();
    let mut eraser = Eraser::new();
    eraser.add_suite::<Sodium>();
    let pwbox = Sodium::build_box(&mut rng)
        .seal(pass_phrase, secret_key)
        .map_err(|_| Error::new(ErrorKind::Other, "Couldn't create a pw box"))?;
    let encrypted_key = eraser
        .erase(pwbox)
        .map_err(|_| Error::new(ErrorKind::Other, "Couldn't convert a pw box"))?;

    Ok(EncryptedKeys {
        public_key,
        secret_key: encrypted_key,
    })
}

fn decrypt(keys: &EncryptedKeys, pass_phrase: &[u8]) -> Result<Vec<u8>, Error> {
    let mut eraser = Eraser::new();
    eraser.add_suite::<Sodium>();
    let restored = match eraser.restore(&keys.secret_key) {
        Ok(restored) => restored,
        Err(_) => {
            return Err(Error::new(
                ErrorKind::Other,
                "Couldn't restore a secret key",
            ))
        }
    };
    Ok(restored
        .open(pass_phrase)
        .map_err(|_| Error::new(ErrorKind::Other, "Couldn't open an encrypted key"))?
        .to_vec())
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

        let encrypt_key = {
            let sk_bytes = hex::decode(sk.to_hex()).expect("Couldn't decode secret key");
            encrypt(pk.to_hex(), &sk_bytes, pass_phrase).expect("Couldn't encrypt keys")
        };

        let decrypted_key = {
            let sk_bytes = decrypt(&encrypt_key, pass_phrase).expect("Couldn't decrypt key");
            SecretKey::from_slice(&sk_bytes).unwrap()
        };

        assert_eq!(sk, decrypted_key);
    }

    #[test]
    fn test_decrypt_from_file() {
        let pass_phrase = b"passphrase";
        let file_content = r#"
public_key = '4642dd43a3489ad0b252c79156ec4beac8e9d59a2d3561d56ce34ef7b363bd64'

[secret_key]
ciphertext = '581a2e85801ccd6f2e019ec78c3c68cfa51dd7ddc08b66b749844ba3582443d564a0e6cfaf098233e3858f4554ed7f7f920f1715a91062d450db9c5e773001c3'
mac = 'f909940a0d45eea96462ce6f20336503'
kdf = 'scrypt-nacl'
cipher = 'xsalsa20-poly1305'

[secret_key.kdfparams]
salt = '748dd73abf954c2796d241ca162f28033a36af80801e4a6045c3d1a9a26170a3'
memlimit = 16777216
opslimit = 524288

[secret_key.cipherparams]
iv = '30d22938dfdb63c3ce2f629b8cfafa35be695858456863fb'
        "#;

        let encrypt_key: EncryptedKeys =
            toml::from_str(file_content).expect("Couldn't deserialize content");
        let decrypted_secret_key = {
            let sk_bytes = decrypt(&encrypt_key, pass_phrase).expect("Couldn't decrypt key");
            SecretKey::from_slice(&sk_bytes).unwrap()
        };

        assert_eq!(
            encrypt_key.public_key,
            "4642dd43a3489ad0b252c79156ec4beac8e9d59a2d3561d56ce34ef7b363bd64"
        );
        assert_eq!(decrypted_secret_key.to_hex(),
"46803f1c86c4c7e0edba803488e10e95d83c83f8b7d95412af9e2f84956cd4b94642dd43a3489ad0b252c79156ec4beac8e9d59a2d3561d56ce34ef7b363bd64")
    }
}
