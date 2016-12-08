// To avoid clippy failure concerning unused mut in cases when it's required
#![allow(unused_mut)]
extern crate serde_json;

use byteorder::{ByteOrder, LittleEndian};

pub mod config_file;
pub mod txs;
pub mod view;
pub mod db;

pub type ConfigurationData = Vec<u8>;
pub type HeightBytecode = [u8; 8];

pub fn height_to_slice(height: u64) -> HeightBytecode {
    let mut result = [0; 8];
    LittleEndian::write_u64(&mut result[0..], height);
    result
}