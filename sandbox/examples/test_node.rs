extern crate exonum;
extern crate timestamping;
extern crate sandbox;
extern crate env_logger;
extern crate clap;

use std::path::Path;

use clap::{Arg, App, SubCommand};

use exonum::config::ConfigFile;
use exonum::node::{Node, Configuration};
use exonum::node::config::GenesisConfig;
use exonum::storage::{MemoryDB, LevelDB, LevelDBOptions};
use exonum::blockchain::Blockchain;
use timestamping::TimestampingBlockchain;

fn run_node<B: Blockchain>(blockchain: B, node_cfg: Configuration) {
    let mut node = Node::new(blockchain, node_cfg);
    node.run().unwrap();
}

fn main() {
    env_logger::init().unwrap();

    let app = App::new("Testnet validator node")
        .version("0.1")
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Test network validator")
        .arg(Arg::with_name("CONFIG")
            .short("c")
            .long("config")
            .value_name("CONFIG_PATH")
            .help("Sets a testnet config file")
            .required(true)
            .takes_value(true))
        .subcommand(SubCommand::with_name("generate")
            .about("Generates default configuration file")
            .version("0.1")
            .author("Aleksey S. <aleksei.sidorov@xdev.re>")
            .arg(Arg::with_name("START_PORT")
                .short("p")
                .long("port")
                .value_name("START_PORT")
                .help("Port for first validator")
                .takes_value(true))
            .arg(Arg::with_name("COUNT")
                .help("Validators count")
                .required(true)
                .index(1)))
        .subcommand(SubCommand::with_name("run")
            .about("Run test node with the given validator id")
            .version("0.1")
            .author("Aleksey S. <aleksei.sidorov@xdev.re>")
            .arg(Arg::with_name("LEVELDB_PATH")
                .short("d")
                .long("leveldb-path")
                .value_name("LEVELDB_PATH")
                .help("Use leveldb database with the given path")
                .takes_value(true))
            .arg(Arg::with_name("PEERS")
                .short("p")
                .long("known-peers")
                .value_name("PEERS")
                .help("Comma separated list of known validator ids")
                .takes_value(true))
            .arg(Arg::with_name("VALIDATOR")
                .help("Sets a validator id")
                .required(true)
                .index(1)));

    let matches = app.get_matches();
    let path = Path::new(matches.value_of("CONFIG").unwrap());
    match matches.subcommand() {
        ("generate", Some(matches)) => {
            let count: u8 = matches.value_of("COUNT").unwrap().parse().unwrap();
            let port: Option<u16> = matches.value_of("START_PORT").map(|x| x.parse().unwrap());
            let cfg = GenesisConfig::gen(count, port);
            ConfigFile::save(&cfg, &path).unwrap();
            println!("The configuration was successfully written to file {:?}",
                     path);
        }
        ("run", Some(matches)) => {
            let cfg: GenesisConfig = ConfigFile::load(path).unwrap();
            let idx: usize = matches.value_of("VALIDATOR").unwrap().parse().unwrap();
            let peers = match matches.value_of("PEERS") {
                Some(string) => {
                    string.split(" ")
                        .map(|x| -> usize { x.parse().unwrap() })
                        .map(|x| cfg.validators[x].address)
                        .collect()
                }
                None => {
                    cfg.validators
                        .iter()
                        .map(|v| v.address)
                        .collect()
                }
            };
            println!("Known peers is {:#?}", peers);
            let node_cfg = cfg.to_node_configuration(idx, peers);
            match matches.value_of("LEVELDB_PATH") {
                Some(ref db_path) => {
                    println!("Using levedb storage with path: {}", db_path);
                    let mut options = LevelDBOptions::new();
                    options.create_if_missing = true;
                    let leveldb = LevelDB::new(&Path::new(db_path), options).unwrap();

                    let blockchain = TimestampingBlockchain { db: leveldb };
                    run_node(blockchain, node_cfg);
                }
                None => {
                    println!("Using memorydb storage");

                    let blockchain = TimestampingBlockchain { db: MemoryDB::new() };
                    run_node(blockchain, node_cfg);                    
                }
            };
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}