
extern crate exonum;
extern crate timestamping;
extern crate sandbox;
extern crate env_logger;
extern crate clap;
extern crate rand;

use std::path::Path;

use clap::{Arg, App, SubCommand};
use rand::{thread_rng, Rng};

use exonum::storage::{MemoryDB};
use sandbox::testnet::{ConfigEntry, TxGeneratorConfiguration, TestnetConfiguration, TxGeneratorNode};
use sandbox::TimestampingTxGenerator;
use timestamping::TimestampingBlockchain;

fn main() {
    ::std::env::set_var("RUST_LOG", "sandbox=debug");
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
            .arg(Arg::with_name("COUNT")
                .help("transactions count")
                .required(true)
                .index(1)));

    let matches = app.get_matches();
    let path = Path::new(matches.value_of("CONFIG").unwrap());
    match matches.subcommand() {
        ("run", Some(matches)) => {
            let cfg = TestnetConfiguration::from_file(path).unwrap();
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

            let mut node_cfg = TxGeneratorConfiguration::new();
            node_cfg.tx_package_size = matches.value_of("TX_PACKAGE").unwrap_or("1000").parse().unwrap();
            node_cfg.tx_timeout = matches.value_of("TX_TIMEOUT").unwrap_or("1000").parse().unwrap();
            let tx_size = matches.value_of("TX_SIZE").unwrap_or("64").parse().unwrap();

            let mut node: TxGeneratorNode<TimestampingBlockchain<MemoryDB>> = TxGeneratorNode::new(node_cfg);
            let node_gen = TimestampingTxGenerator::new(tx_size)
                .map(|x| (*thread_rng().choose(peers.as_slice()).unwrap(), x))
                .take(count);
            node.append_transactions(node_gen);
            node.run(&peers);
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}
