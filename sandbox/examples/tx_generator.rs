#[macro_use]
extern crate log;

extern crate exonum;
extern crate timestamping;
extern crate sandbox;
extern crate env_logger;
extern crate clap;

use std::path::Path;
use std::{thread, time};
use std::cmp::min;

use clap::{Arg, App, SubCommand};

use exonum::node::{Node};
use exonum::storage::{MemoryDB};

use timestamping::TimestampingBlockchain; 

use sandbox::config_file::ConfigFile;
use sandbox::config::NodeConfig;
use sandbox::TimestampingTxGenerator;

fn main() {
    env_logger::init().unwrap();

    let app = App::new("Testnet transaction generator")
        .version("0.1")
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Test network node")
        .arg(Arg::with_name("CONFIG")
            .short("c")
            .long("config")
            .value_name("CONFIG_PATH")
            .help("Sets a testnet config file")
            .required(true)
            .takes_value(true))
        .subcommand(SubCommand::with_name("run")
            .about("Run transaction generator")
            .version("0.1")
            .author("Aleksey S. <aleksei.sidorov@xdev.re>")
            .arg(Arg::with_name("PEERS")
                .short("p")
                .long("known-peers")
                .value_name("PEERS")
                .help("Comma separated list of known validator ids")
                .takes_value(true))
            .arg(Arg::with_name("TX_PACKAGE")
                .short("x")
                .long("tx-package-size")
                .value_name("TX_PACKAGE")
                .help("A size of one package"))
            .arg(Arg::with_name("TX_TIMEOUT")
                .short("t")
                .long("tx-timeout")
                .value_name("TX_TIMEOUT")
                .help("A duration between packages"))
            .arg(Arg::with_name("TX_SIZE")
                .short("d")
                .long("tx-data-size")
                .value_name("TX_SIZE")
                .help("A transaction data size"))
            .arg(Arg::with_name("VALIDATOR")
                .help("Sets a validator id")
                .required(true)
                .index(1))
            .arg(Arg::with_name("COUNT")
                .help("transactions count")
                .required(true)
                .index(2)));

    let matches = app.get_matches();
    let path = Path::new(matches.value_of("CONFIG").unwrap());
    match matches.subcommand() {
        ("run", Some(matches)) => {
            let cfg: NodeConfig = ConfigFile::load(path).unwrap();
            let idx: usize = matches.value_of("VALIDATOR").unwrap().parse().unwrap();
            let count: usize = matches.value_of("COUNT").unwrap().parse().unwrap();
            let peers = match matches.value_of("PEERS") {
                Some(string) => {
                    string.split(" ")
                        .map(|x| -> usize { x.parse().unwrap() })
                        .map(|x| cfg.validators[x].address)
                        .collect::<Vec<_>>()
                }
                None => {
                    cfg.validators
                        .iter()
                        .map(|v| v.address)
                        .collect::<Vec<_>>()
                }
            };

            let node_cfg = cfg.to_node_configuration(idx, peers);

            let blockchain = TimestampingBlockchain { db: MemoryDB::new() };
            let mut node = Node::new(blockchain.clone(), node_cfg);
            let chan = node.channel();

            let tx_package_size: usize = matches.value_of("TX_PACKAGE").unwrap_or("1000").parse().unwrap();
            let tx_timeout: u64 = matches.value_of("TX_TIMEOUT").unwrap_or("1000").parse().unwrap();
            let tx_size = matches.value_of("TX_SIZE").unwrap_or("64").parse().unwrap();
            let mut tx_gen = TimestampingTxGenerator::new(tx_size);

            let handle = thread::spawn(move || {
                let gen = &mut tx_gen;
                let mut tx_remaining = count;
                while tx_remaining > 0 {                    
                    let count = min(tx_remaining, tx_package_size);
                    for tx in gen.take(count) {
                        chan.send(tx);
                    }
                    tx_remaining -= count;
                    
                    println!("There are {} transactions in the pool",
                        tx_remaining);

                    let timeout = time::Duration::from_millis(tx_timeout);
                    thread::sleep(timeout);   
                }                 
            });

            node.run().unwrap();
            handle.join().unwrap();
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}