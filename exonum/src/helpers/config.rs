use serde::{Serialize, Deserialize};
use toml;

use std::path::Path;
use std::io;
use std::fs;
use std::error::Error;
use std::io::prelude::*;
use std::fs::File;

#[derive(Debug)]
pub struct ConfigFile {}

impl ConfigFile {
    pub fn load<P, T>(path: P) -> Result<T, Box<Error>>
    where
        T: for<'r> Deserialize<'r>,
        P: AsRef<Path>,
    {
        let mut file = File::open(path.as_ref())?;
        let mut toml = String::new();
        file.read_to_string(&mut toml)?;
        toml::de::from_str(&toml).map_err(|_| {
            let e = io::Error::new(io::ErrorKind::InvalidData, "Unable to decode toml file");
            Box::new(e) as Box<Error>
        })
    }

    pub fn save<P, T>(value: &T, path: P) -> Result<(), Box<Error>>
    where
        T: Serialize,
        P: AsRef<Path>,
    {
        if let Some(dir) = path.as_ref().parent() {
            fs::create_dir_all(dir)?;
        }

        let mut file = File::create(path.as_ref())?;
        let value_toml = toml::Value::try_from(value)?;
        file.write_all(&format!("{}", value_toml).into_bytes())?;

        Ok(())
    }
}
