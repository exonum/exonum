use serde::{Serialize, Deserialize};
use toml::{self, Encoder};

use std::path::Path;
use std::io;
use std::fs;
use std::error::Error;
use std::io::prelude::*;
use std::fs::File;

pub struct ConfigFile {}

impl ConfigFile {
    pub fn load<T: Deserialize>(path: &Path) -> Result<T, Box<Error>> {
        let mut file = File::open(path)?;
        let mut toml = String::new();
        file.read_to_string(&mut toml)?;
        toml::decode_str(&toml).ok_or_else(|| {
                                               let e = io::Error::new(io::ErrorKind::InvalidData,
                                                                      "Unable to decode toml file");
                                               Box::new(e) as Box<Error>
                                           })
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
