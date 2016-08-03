extern crate da;
extern crate env_logger;

use da::node::{Node, Configuration};
use da::network::{NetworkConfiguration};
use da::events::{EventsConfiguration};
use da::crypto::{gen_keypair_from_seed, Seed};


fn main() {

    let mut idx: usize = ::std::env::args().last().unwrap().parse().unwrap();
    idx -= 1;

    ::std::env::set_var("RUST_LOG", "da=info");

    env_logger::init().unwrap();

    let addresses : Vec<::std::net::SocketAddr> = vec![
        "127.0.0.1:7000".parse().unwrap(),
        "127.0.0.1:7001".parse().unwrap(),
        "127.0.0.1:7002".parse().unwrap(),
        "127.0.0.1:7003".parse().unwrap(),
    ];

    let pairs = vec![
        gen_keypair_from_seed(&Seed::from_slice(&vec![0; 32]).unwrap()),
        gen_keypair_from_seed(&Seed::from_slice(&vec![1; 32]).unwrap()),
        gen_keypair_from_seed(&Seed::from_slice(&vec![2; 32]).unwrap()),
        gen_keypair_from_seed(&Seed::from_slice(&vec![3; 32]).unwrap()),
    ];

    let validators : Vec<_> = pairs.iter()
                                   .map(|&(ref p, _)| p.clone())
                                   .collect();

    let address = addresses[idx];
    let (ref public_key, ref secret_key) = pairs[idx];

    Node::with_config(Configuration {
        public_key: public_key.clone(),
        secret_key: secret_key.clone(),
        round_timeout: 2000,
        network: NetworkConfiguration {
            listen_address: address.clone(),
            max_incoming_connections: 8,
            max_outgoing_connections: 8,
        },
        events: EventsConfiguration::new(),
        peer_discovery: addresses.clone(),
        validators: validators.clone(),
    }).run();
}
