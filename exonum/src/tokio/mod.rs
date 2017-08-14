pub mod codec;
pub mod error;
pub mod handler;
pub mod network;

use router::Router;
use mount::Mount;
use iron::{Chain, Iron};
use toml::Value;
use tokio_core::reactor::Core;

use std::io;
use std::thread;

use crypto::{self, PublicKey, SecretKey, Hash};
use events::{Events, Reactor, NetworkConfiguration, Event, EventsConfiguration, Channel,
             MioChannel, Network, EventLoop, Milliseconds, EventHandler, Result as EventsResult,
             Error as EventsError};
use blockchain::{SharedNodeState, Blockchain, Schema, GenesisConfig, Transaction, ApiContext};
use messages::{Connect, RawMessage};
use api::{Api, public, private};
use node::{NodeConfig, Configuration, ListenerConfig, ServiceConfig, NodeHandler, NodeApiConfig,
           ApiSender};
use node::state::{State, Round, Height, RequestData, ValidatorId, TxPool, ValidatorState};

use self::handler::{NodeSender, NodeReceiver, NodeChannel};
use self::network::run_node;

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


        let mut events_cfg = EventsConfiguration::default();
        events_cfg.notify_capacity(node_cfg.mempool.events_pool_capacity);
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
            events: events_cfg,
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

    /// A generic implementation that launches `Node` and optionally creates threads
    /// for public and private api handlers.
    /// Explorer api prefix is `/api/explorer`
    /// Public api prefix is `/api/services/{service_name}`
    /// Private api prefix is `/api/services/{service_name}`
    pub fn run(self) -> io::Result<()> {
        run_node(self)
    }

    /// Returns channel.
    pub fn channel(&self) -> ApiSender<NodeSender> {
        ApiSender::new(self.channel.0.clone())
    }
}
