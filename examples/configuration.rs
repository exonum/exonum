extern crate iron;
extern crate env_logger;
extern crate clap;
extern crate serde;
extern crate serde_json;
extern crate bodyparser;

extern crate exonum;
extern crate router;
extern crate configuration_service;

use std::net::SocketAddr;
use clap::{Arg, App};

use exonum::blockchain::{Blockchain, Service};
use exonum::node::Node;
use exonum::helpers::clap::{GenerateCommand, RunCommand};
use exonum::helpers::{run_node_with_api, NodeRunOptions};

use configuration_service::ConfigurationService;

fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().unwrap();

    let app = App::new("Simple configuration api demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Demo validator node")
        .subcommand(GenerateCommand::new())
        .subcommand(RunCommand::new()
                        .arg(Arg::with_name("CFG_PUB_HTTP_PORT")
                                 .short("p")
                                 .long("public-port")
                                 .value_name("CFG_PUB_HTTP_PORT")
                                 .help("Run public config api http server on given port")
                                 .takes_value(true))
                        .arg(Arg::with_name("CFG_PRIV_HTTP_PORT")
                                 .short("s")
                                 .long("private-port")
                                 .value_name("CFG_PRIV_HTTP_PORT")
                                 .help("Run config api http server on given port")
                                 .takes_value(true)));
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => GenerateCommand::execute(matches),
        ("run", Some(matches)) => {
            let pub_port: Option<u16> = matches
                .value_of("CFG_PUB_HTTP_PORT")
                .map(|x| x.parse().unwrap());
            let priv_port: Option<u16> = matches
                .value_of("CFG_PRIV_HTTP_PORT")
                .map(|x| x.parse().unwrap());
            let node_cfg = RunCommand::node_config(matches);
            let db = RunCommand::db(matches);

            let services: Vec<Box<Service>> = vec![Box::new(ConfigurationService::new())];
            let blockchain = Blockchain::new(db, services);

            let node = Node::new(blockchain, node_cfg);
            let opts = NodeRunOptions {
                enable_explorer: true,
                public_api_address: pub_port.map(|port| SocketAddr::from(([127, 0, 0, 1], port))),
                private_api_address: priv_port.map(|port| SocketAddr::from(([127, 0, 0, 1], port))),
            };
            run_node_with_api(node, opts);
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}
