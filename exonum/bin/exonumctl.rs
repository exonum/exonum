extern crate exonum;
extern crate env_logger;
extern crate clap;

use clap::{App, SubCommand, Arg, ArgMatches};

use std::marker::PhantomData;
use std::path::Path;

use exonum::storage::{RocksDB, RocksDBOptions, Map, MapIndex};
use exonum::crypto::{HexValue, Hash};
use exonum::helpers::clap::GenerateCommand;

pub enum BlockchainAction {
    FindTx(String),
}

pub struct BlockchainCommand<'a, 'b>
    where 'a: 'b
{
    _p: PhantomData<App<'a, 'b>>,
}

impl<'a, 'b> BlockchainCommand<'a, 'b>
    where 'a: 'b
{
    pub fn new() -> App<'a, 'b> {
        SubCommand::with_name("blockchain")
            .about("Blockchain explorer")
            .arg(Arg::with_name("DB_PATH")
                     .short("d")
                     .long("db-path")
                     .value_name("DB_PATH")
                     .help("Use database with the given path")
                     .required(true)
                     .takes_value(true))
            .subcommand(SubCommand::with_name("find_tx")
                            .about("Find transaction with given hash")
                            .arg(Arg::with_name("TX_HASH")
                                     .help("Transaction hash")
                                     .required(true)
                                     .index(1)))
    }

    pub fn db_path(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("DB_PATH").unwrap())
    }

    pub fn db(matches: &'a ArgMatches<'a>) -> RocksDB {
        let path = Self::db_path(matches);
        let mut options = RocksDBOptions::default();
        options.create_if_missing(true);
        RocksDB::open(path, options).unwrap()
    }

    pub fn action(matches: &'a ArgMatches<'a>) -> BlockchainAction {
        match matches.subcommand() {
            ("find_tx", Some(matches)) => {
                let tx_hash = matches.value_of("TX_HASH").unwrap();
                BlockchainAction::FindTx(tx_hash.into())
            }
            _ => unreachable!("Wrong subcommand"),
        }
    }

    pub fn execute(matches: &'a ArgMatches<'a>) {
        let storage = Self::db(matches);
        match Self::action(matches) {
            BlockchainAction::FindTx(tx_hash) => {
                let hash = Hash::from_hex(tx_hash).unwrap();
                // TODO add constants to common used prefixes in blockchain
                let txs = MapIndex::new(vec![00], &storage);
                let tx: Vec<u8> = txs.get(&hash).unwrap().unwrap();
                println!("{}", tx.to_hex());
            }
        }
    }
}


fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().unwrap();

    let app = App::new("Blockchain control utility")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdevre>")
        .about("Blockchain control utility")
        .subcommand(GenerateCommand::new())
        .subcommand(BlockchainCommand::new());

    let matches = app.get_matches();
    match matches.subcommand() {
        ("generate", Some(matches)) => GenerateCommand::execute(matches),
        ("blockchain", Some(matches)) => BlockchainCommand::execute(matches),
        _ => unreachable!("Wrong subcommand"),
    }
}
