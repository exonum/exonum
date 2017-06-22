use std::path::Path;
use std::net::SocketAddr;

use storage::Storage;
use blockchain::{Service, Blockchain};
use node::{Node, NodeConfig};
use config::ConfigFile;

use super::internal::{CollectedCommand, Feedback};
use super::clap_backend::ClapBackend;
use super::{Context, ServiceFactory};
use super::details::{GenerateTestnetCommand, RunCommand,
                     KeyGeneratorCommand, GenerateTemplateCommand};
/// `NodeBuilder` is a high level object,
/// usable for fast prototyping and creating app from services list.

pub struct NodeBuilder {
    commands: Vec<CollectedCommand>,
    service_constructors: Vec<Box<FnMut(&Context) -> Box<Service>>>,
}

impl NodeBuilder {

    pub fn new() -> NodeBuilder {
        NodeBuilder {
            commands: vec![CollectedCommand::new(Box::new(GenerateTestnetCommand)),
                           CollectedCommand::new(Box::new(RunCommand)),
                           CollectedCommand::new(Box::new(KeyGeneratorCommand)),
                           CollectedCommand::new(Box::new(GenerateTemplateCommand))
                           ],
            service_constructors: Vec::new()
        }
    }

    pub fn with_service<S: ServiceFactory>(mut self, service: S) -> NodeBuilder {
        //TODO: take endpoints, etc...

        for ref mut command in self.commands.iter_mut() {
            let name = command.name();
            command.extend(S::command(name))
        }
        self.service_constructors.push(Self::make_constructor(service));
        self
    }

    fn make_constructor<S>(service: S) ->
        Box<FnMut(&Context) -> Box<Service>>
        where S: ServiceFactory
    {
        //TODO: wait for `FnBox` to be stable 
        // https://github.com/rust-lang/rust/issues/28796
        let mut service = Some(service);
        Box::new(move |context| service.take()
                                       .expect("Service constructor called twice.")
                                       .make_service(context))
    }

    #[cfg(not(feature="memorydb"))]
    pub fn db_helper(ctx: &Context) -> Storage {
        use storage::{LevelDB, LevelDBOptions};

        let path = ctx.get::<String>("LEVELDB_PATH")
                      .expect("LEVELDB_PATH not found.");
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        LevelDB::new(Path::new(&path), options).unwrap()
    }

    #[cfg(feature="memorydb")]
    pub fn db_helper(_: &Context) -> Storage {
        use storage::MemoryDB;
        MemoryDB::new()
    }

    pub fn node_config(ctx: &Context) -> NodeConfig {
        let path = ctx.get::<String>("NODE_CONFIG_PATH")
                      .expect("NODE_CONFIG_PATH not found.");
        let mut cfg: NodeConfig = ConfigFile::load(Path::new(&path)).unwrap();
        // Override api options
        if let Some(addr) = Self::public_api_address(ctx) {
            cfg.api.public_api_address = Some(addr);
        }
        if let Some(addr) = Self::private_api_address(ctx) {
            cfg.api.private_api_address = Some(addr);
        }
        cfg
    }

    pub fn public_api_address(ctx: &Context) -> Option<SocketAddr> {
        ctx.get::<String>("PUBLIC_API_ADDRESS").ok()
            .map(|s|
                s.parse()
                 .expect("Public api address has incorrect format"))
    }

    pub fn private_api_address(ctx: &Context) -> Option<SocketAddr> {
        ctx.get::<String>("PRIVATE_API_ADDRESS").ok()
            .map(|s|
                s.parse()
                 .expect("Public api address has incorrect format"))
    }

    pub fn parse_cmd(self) -> Option<Node> {
        match ClapBackend::execute(self.commands.as_slice()) {
            Feedback::RunNode(ref ctx) => {
                let db = Self::db_helper(ctx);
                let config = Self::node_config(ctx);
                let services: Vec<Box<Service>> = self.service_constructors
                                                      .into_iter()
                                                      .map(|mut constructor| constructor(ctx))
                                                      .collect();
                let blockchain = Blockchain::new(db, services);
                let node = Node::new(blockchain, config);
                Some(node)
            }
            _ => None
        }
    }

    pub fn run(self) {
        if let Some(mut node) = self.parse_cmd() {
            node.run().expect("Node return error")
        }
    }
}
