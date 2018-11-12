#![allow(dead_code)]
use super::{gen_keypair, hash, PublicKey, SecretKey};
use openssl::symm::{decrypt, encrypt, Cipher};
use pem::{encode, parse, Pem};
#[cfg(not(windows))]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs::{File, OpenOptions},
    io::Error,
    io::Read,
    io::Write,
    path::Path,
};
const DEFAULT_KEY_PATH: &str = "../private_key_file.pem";
const IV: Option<&[u8]> = Some(b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07");

enum KeyOperation {
    ENCRYPT,
    DECRYPT,
}

/// Creates a PEM file that contains encrypted `SecretKey` and returns `PublicKey` for the secret key.
pub fn create_pem_file(path: Option<&str>, pass_phrase: &[u8]) -> Result<PublicKey, Error> {
    let (pk, sk) = gen_keypair();
    let sk_bytes = hex::decode(sk.to_hex()).expect("Couldn't decode secret key in bytes");
    let enc_sk = key_processing(pass_phrase, KeyOperation::ENCRYPT, &sk_bytes)?;
    let pem = Pem {
        tag: "PRIVATE KEY".to_string(),
        contents: enc_sk,
    };

    let mut pem_file = if cfg!(target_os = "windows") {
        File::create(path.unwrap_or(DEFAULT_KEY_PATH))?
    } else {
        OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o600)
            .open(path.unwrap_or(DEFAULT_KEY_PATH))?
    };
    pem_file.write_all(encode(&pem).as_bytes())?;

    Ok(pk)
}

/// Reads `SecretKey` from encrypted file located by path and returns it.
pub fn read_key_from_pem<P: AsRef<Path>>(path: P, pass_phrase: &[u8]) -> Result<SecretKey, Error> {
    let mut key_file = File::open(path)?;
    let mut pem_data = vec![];
    key_file.read_to_end(&mut pem_data)?;
    let pem = parse(pem_data).unwrap();
    let dec_sk = key_processing(pass_phrase, KeyOperation::DECRYPT, &pem.contents)?;
    let sk = SecretKey::from_slice(&dec_sk).unwrap();

    Ok(sk)
}

fn key_processing(
    pass_phrase: &[u8],
    operation: KeyOperation,
    data: &[u8],
) -> Result<Vec<u8>, Error> {
    let cipher = Cipher::aes_256_cbc();
    let key = hash(pass_phrase);

    match operation {
        KeyOperation::ENCRYPT => encrypt(cipher, key.as_ref(), IV, data).map_err(|e| e.into()),
        KeyOperation::DECRYPT => decrypt(cipher, key.as_ref(), IV, data).map_err(|e| e.into()),
    }
}

#[test]
fn test_create_and_read_pem_file() {
    use std::fs::remove_file;
    let path = "/tmp/private_key.pem";
    let pass_phrase = b"passphrase";

    assert!(create_pem_file(Some(path), pass_phrase).is_ok());
    assert!(read_key_from_pem(path, pass_phrase).is_ok());
    remove_file(path).unwrap();

    assert!(create_pem_file(None, pass_phrase).is_ok());
    assert!(read_key_from_pem(DEFAULT_KEY_PATH, pass_phrase).is_ok());
    remove_file(DEFAULT_KEY_PATH).unwrap();
}

#[test]
fn test_encrypt_decrypt() {
    let pass_phrase = b"passphrase";
    let (_, sk) = gen_keypair();

    let encrypt_key = {
        let sk_bytes = hex::decode(sk.to_hex()).expect("Couldn't decode secret key");
        key_processing(pass_phrase, KeyOperation::ENCRYPT, &sk_bytes).unwrap()
    };

    let decrypted_key = {
        let sk_bytes = key_processing(pass_phrase, KeyOperation::DECRYPT, &encrypt_key).unwrap();
        SecretKey::from_slice(&sk_bytes).unwrap()
    };

    assert_eq!(sk, decrypted_key);
}
