#[macro_use]
extern crate log;

extern crate exonum;
extern crate blockchain_explorer;
extern crate timestamping;
extern crate sandbox;
extern crate clap;

use std::{thread, time};
use std::cmp::min;

use clap::{Arg, App};

use exonum::node::{Node, NodeConfig};
use exonum::storage::Database;
use exonum::blockchain::Blockchain;

use timestamping::TimestampingService;
use blockchain_explorer::helpers::RunCommand;

use sandbox::TimestampingTxGenerator;

struct TxGeneratorConfig {
    tx_size: usize,
    tx_count: usize,
    tx_package_size: usize,
    tx_timeout: u64,
}

fn run_node(blockchain: Blockchain, node_cfg: NodeConfig, tx_gen_cfg: TxGeneratorConfig) {
    let mut node = Node::new(blockchain.clone(), node_cfg);
    let chan = node.channel();
    let mut tx_gen = TimestampingTxGenerator::new(tx_gen_cfg.tx_size);

    let handle = thread::spawn(move || {
        let gen = &mut tx_gen;
        let mut tx_remaining = tx_gen_cfg.tx_count;
        while tx_remaining > 0 {
            let count = min(tx_remaining, tx_gen_cfg.tx_package_size);
            for tx in gen.take(count) {
                if let Err(e) = chan.send(tx) {
                    trace!("Unable to add tx to node channel error={:?}", e);
                }
            }
            tx_remaining -= count;

            println!("There are {} transactions in the pool", tx_remaining);

            let timeout = time::Duration::from_millis(tx_gen_cfg.tx_timeout);
            thread::sleep(timeout);
        }
    });

    node.run().unwrap();
    handle.join().unwrap();
}

fn main() {
    exonum::crypto::init();
    blockchain_explorer::helpers::init_logger().unwrap();

    let app = App::new("Testnet transaction generator")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Test network node")
        .subcommand(RunCommand::new()
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
    match matches.subcommand() {
        ("run", Some(matches)) => {
            let tx_gen_cfg = TxGeneratorConfig {
                tx_size: matches.value_of("TX_SIZE").unwrap_or("64").parse().unwrap(),
                tx_count: matches.value_of("COUNT").unwrap().parse().unwrap(),
                tx_package_size: matches.value_of("TX_PACKAGE").unwrap_or("1000").parse().unwrap(),
                tx_timeout: matches.value_of("TX_TIMEOUT").unwrap_or("1000").parse().unwrap(),
            };
            let node_cfg = RunCommand::node_config(matches);
            let db = RunCommand::db(matches);

            let blockchain = Blockchain::new(db, vec![Box::new(TimestampingService::new())]);
            run_node(blockchain, node_cfg, tx_gen_cfg);
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}
