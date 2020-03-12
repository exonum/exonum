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

use exonum_keys::*;
use hex;
use serde_json::json;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "key-generator",
    about = "An utility for keys generation from a seed."
)]
struct Arguments {
    /// Seed for deriving keys.
    #[structopt(short = "s", long)]
    seed: String,

    /// Passphrase for encrypting the seed.
    #[structopt(short = "p", long)]
    passphrase: String,
}

fn main() {
    let args = Arguments::from_args();
    let result = generate_json(&args.passphrase, &args.seed).unwrap();

    println!(
        "{}",
        serde_json::to_string_pretty(&result).expect("Couldn't convert json object to string")
    );
}

fn generate_json(passphrase: &str, seed: &str) -> anyhow::Result<serde_json::Value> {
    let seed = hex::decode(seed)?;
    let (keys, encrypted_key) = generate_keys_from_seed(passphrase.as_bytes(), &seed)?;
    let file_content = toml::to_string_pretty(&encrypted_key)?;

    Ok(json!({
        "consensus_pub_key": keys.consensus.public_key().to_hex(),
        "service_pub_key": keys.service.public_key().to_hex(),
        "master_key_file": file_content,
    }))
}

#[test]
fn test_key_generator() {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    use std::{fs::OpenOptions, io::Write};
    use tempdir::TempDir;

    let tempdir = TempDir::new("test_key_generator").unwrap();
    let master_key_path = tempdir.path().join("master_key.toml");
    let seed = "a7839ea524f38d0e91a5ec96a723092719dc8a5b8a75f9131d9eb38f45e76344";
    let passphrase = "passphrase";

    let json = generate_json(passphrase, seed).unwrap();

    let mut open_options = OpenOptions::new();
    open_options.create(true).write(true);
    // By agreement we use the same permissions as for SSH private keys.
    #[cfg(unix)]
    open_options.mode(0o_600);
    let mut file = open_options.open(&master_key_path).unwrap();
    file.write_all(
        json.get("master_key_file")
            .unwrap()
            .as_str()
            .unwrap()
            .as_bytes(),
    )
    .unwrap();

    let r_keys = read_keys_from_file(&master_key_path, passphrase).unwrap();

    assert_eq!(
        json["service_pub_key"].as_str().unwrap(),
        r_keys.service.public_key().to_hex()
    );
    assert_eq!(
        json["consensus_pub_key"].as_str().unwrap(),
        r_keys.consensus.public_key().to_hex()
    );
}
