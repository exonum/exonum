extern crate exonum;
extern crate env_logger;
extern crate rand;
extern crate clap;

use std::path::Path;

use clap::{Arg, App, SubCommand};

use exonum::storage::{LevelDB, LevelDBOptions, MapTable, Map};
use exonum::crypto::{Hash, HexValue};

/// usage
/// path  - Directory where database is situated
/// count - Total amount of data items to write
/// data_len - Length of data chunk
/// seed - seed for rng

fn main() {
    let app = App::new("Blockchain utils")
        .version("0.1")
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("An utils to find useful values in blockchain")
        .arg(Arg::with_name("LEVELDB_PATH")
            .short("d")
            .long("leveldb-path")
            .value_name("LEVELDB_PATH")
            .help("Use leveldb database with the given path")
            .required(true)
            .takes_value(true))
        .subcommand(SubCommand::with_name("find_tx")
            .about("Find transaction with given hash")
            .version("0.1")
            .author("Aleksey S. <aleksei.sidorov@xdev.re>")
            .arg(Arg::with_name("TX_HASH")
                .help("Transaction hash")
                .required(true)
                .index(1)));

    let matches = app.get_matches();
    let path = matches.value_of("LEVELDB_PATH").unwrap();
    let options = LevelDBOptions::new();
    let db = LevelDB::new(&Path::new(&path), options).unwrap();

    match matches.subcommand() {
        ("find_tx", Some(matches)) => {
            let tx_hash = Hash::from_hex(matches.value_of("TX_HASH").unwrap()).unwrap();
            println!("tx_hash: {}", tx_hash.to_hex());
            // TODO add constants to common used prefixes in blockchain
            let txs = MapTable::new(vec![00], &db);
            let tx: Vec<u8> = txs.get(&tx_hash).unwrap().unwrap();
            println!("{}", tx.to_hex());
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}
