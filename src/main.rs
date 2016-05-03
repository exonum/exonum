#![allow(dead_code)]

#![feature(associated_consts)]

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate time;
extern crate byteorder;
extern crate mio;
extern crate sodiumoxide;

mod message;
mod protocol;
mod connection;
mod network;
mod events;
mod crypto;
mod state;
mod node;

use node::{Node, Configuration};
use network::{NetworkConfiguration};
use events::{EventsConfiguration};
use crypto::{gen_keypair};

fn main() {
    env_logger::init().unwrap();

    let addresses : Vec<::std::net::SocketAddr> = vec![
        "127.0.0.1:7000".parse().unwrap(),
        "127.0.0.1:7001".parse().unwrap(),
        "127.0.0.1:7002".parse().unwrap(),
        "127.0.0.1:7003".parse().unwrap(),
    ];

    let pairs = vec![
        gen_keypair(),
        gen_keypair(),
        gen_keypair(),
        gen_keypair(),
    ];

    let validators : Vec<_> = pairs.iter()
                                   .map(|&(ref p, _)| p.clone())
                                   .collect();

    let mut nodes = Vec::new();

    for (address, (public_key, secret_key)) in addresses.iter().zip(pairs) {
        nodes.push(Node::with_config(Configuration {
            public_key: public_key,
            secret_key: secret_key,
            round_timeout: 1000,
            network: NetworkConfiguration {
                listen_address: address.clone(),
                max_incoming_connections: 8,
                max_outgoing_connections: 8,
            },
            events: EventsConfiguration::new(),
            peer_discovery: addresses.clone(),
            validators: validators.clone(),
        }))
    }

    ::std::thread::sleep(::std::time::Duration::from_millis(100));

    let mut threads = Vec::new();
    for mut node in nodes {
        threads.push(::std::thread::spawn(move || {
            node.run()
        }))
    }

    for thread in threads {
        let _ = thread.join();
    }
}
