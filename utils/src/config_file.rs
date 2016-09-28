use std::path::Path;
use std::fs;
use std::error::Error;
use std::io::prelude::*;
use std::fs::File;

use serde::{Serialize, Deserialize};
use toml;
use toml::Encoder;

pub struct ConfigFile {}

impl ConfigFile {
    pub fn load<T: Deserialize>(path: &Path) -> Result<T, Box<Error>> {
        let mut file = File::open(path)?;
        let mut toml = String::new();
        file.read_to_string(&mut toml)?;
        let cfg = toml::decode_str(&toml);
        return Ok(cfg.unwrap());
    }

    pub fn save<T: Serialize>(value: &T, path: &Path) -> Result<(), Box<Error>> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        let mut e = Encoder::new();
        value.serialize(&mut e)?;
        let mut file = File::create(path)?;
        file.write_all(toml::encode_str(&e.toml).as_bytes())?;

        Ok(())
    }
}
