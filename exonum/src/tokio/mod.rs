pub mod codec;
pub mod error;
pub mod handler;
pub mod network;

use router::Router;
use mount::Mount;
use iron::{Chain, Iron};
use tokio_core::reactor::Core;

use std::io;
use std::thread;

use crypto;
use blockchain::{SharedNodeState, Blockchain, ApiContext};
use api::{Api, public, private};
use node::{NodeConfig, Configuration, ListenerConfig, ServiceConfig, NodeHandler, NodeApiConfig,
           ApiSender};
use node::state::State;

use self::network::run_node_handler;
pub use self::handler::{NodeSender, NodeReceiver, NodeChannel};

const PROFILE_ENV_VARIABLE_NAME: &'static str = "EXONUM_PROFILE_FILENAME";

/// Node that contains handler (`NodeHandler`) and `NodeApiConfig`.
#[derive(Debug)]
pub struct Node {
    api_options: NodeApiConfig,
    handler: NodeHandler<NodeSender>,
    channel: NodeChannel,
    core: Core,
}

impl Node {
    /// Creates node for the given blockchain and node configuration.
    pub fn new(mut blockchain: Blockchain, node_cfg: NodeConfig) -> Self {
        crypto::init();

        if cfg!(feature = "flame_profile") {
            ::profiler::init_handler(::std::env::var(PROFILE_ENV_VARIABLE_NAME).expect(&format!(
                "You compiled exonum with profiling support, but {}",
                PROFILE_ENV_VARIABLE_NAME
            )))
        };

        blockchain
            .create_genesis_block(node_cfg.genesis.clone())
            .unwrap();


        let config = Configuration {
            listener: ListenerConfig {
                consensus_public_key: node_cfg.consensus_public_key,
                consensus_secret_key: node_cfg.consensus_secret_key,
                whitelist: node_cfg.whitelist,
                address: node_cfg.listen_address,
            },
            service: ServiceConfig {
                service_public_key: node_cfg.service_public_key,
                service_secret_key: node_cfg.service_secret_key,
            },
            mempool: node_cfg.mempool,
            network: node_cfg.network,
            peer_discovery: node_cfg.peers,
        };

        let external_address = if let Some(v) = node_cfg.external_address {
            v
        } else {
            warn!("Could not find 'external_address' in the config, using 'listen_address'");
            node_cfg.listen_address
        };
        let api_state = SharedNodeState::new(node_cfg.api.state_update_timeout as u64);
        let channel = NodeChannel::new(node_cfg.listen_address, 64);
        let core = Core::new().unwrap();
        let handler = NodeHandler::new(
            blockchain,
            external_address,
            channel.0.clone(),
            config,
            api_state,
            core.handle(),
        );
        Node {
            api_options: node_cfg.api,
            handler,
            channel,
            core,
        }
    }

    /// Launches only consensus messages handler.
    /// This may be used if you want to customize api with the `ApiContext`.
    pub fn run_handler(self) -> io::Result<()> {
        run_node_handler(self)
    }

    /// A generic implementation that launches `Node` and optionally creates threads
    /// for public and private api handlers.
    /// Explorer api prefix is `/api/explorer`
    /// Public api prefix is `/api/services/{service_name}`
    /// Private api prefix is `/api/services/{service_name}`
    pub fn run(self) -> io::Result<()> {
        let blockchain = self.handler().blockchain.clone();
        let channel = self.channel();

        let private_config_api_thread = match self.api_options.private_api_address {
            Some(listen_address) => {
                let api_context = ApiContext::new(&self);
                let mut mount = Mount::new();
                mount.mount("api/services", api_context.mount_private_api());
                let shared_api_state = self.handler().api_state().clone();
                let mut router = Router::new();
                let node_info =
                    private::NodeInfo::new(blockchain.service_map().iter().map(|(_, s)| s));
                let system_api = private::SystemApi::new(
                    node_info,
                    blockchain.clone(),
                    shared_api_state,
                    channel,
                );
                system_api.wire(&mut router);
                mount.mount("api/system", router);

                let thread = thread::spawn(move || {
                    info!("Private exonum api started on {}", listen_address);
                    let chain = Chain::new(mount);
                    Iron::new(chain).http(listen_address).unwrap();
                });
                Some(thread)
            }
            None => None,
        };
        let public_config_api_thread = match self.api_options.public_api_address {
            Some(listen_address) => {
                let api_context = ApiContext::new(&self);
                let mut mount = Mount::new();
                mount.mount("api/services", api_context.mount_public_api());

                let mut router = Router::new();
                let pool = self.state().transactions().clone();
                let system_api = public::SystemApi::new(pool, blockchain.clone());
                system_api.wire(&mut router);
                mount.mount("api/system", router);
                if self.api_options.enable_blockchain_explorer {
                    let mut router = Router::new();
                    let explorer_api = public::ExplorerApi::new(blockchain);
                    explorer_api.wire(&mut router);
                    mount.mount("api/explorer", router);
                }

                let thread = thread::spawn(move || {
                    info!("Public exonum api started on {}", listen_address);

                    let chain = Chain::new(mount);
                    Iron::new(chain).http(listen_address).unwrap();
                });
                Some(thread)
            }
            None => None,
        };

        run_node_handler(self)?;

        if let Some(private_config_api_thread) = private_config_api_thread {
            private_config_api_thread.join().unwrap();
        }
        if let Some(public_config_api_thread) = public_config_api_thread {
            public_config_api_thread.join().unwrap();
        }

        Ok(())
    }

    /// Returns `State`.
    pub fn state(&self) -> &State {
        self.handler.state()
    }

    /// Returns `NodeHandler`.
    pub fn handler(&self) -> &NodeHandler<NodeSender> {
        &self.handler
    }

    /// Returns channel.
    pub fn channel(&self) -> ApiSender {
        ApiSender::new(self.channel.0.external.clone())
    }
}


// New traits

use std::time::SystemTime;

pub trait SystemStateProvider {
    fn current_time(&self) -> SystemTime;
}