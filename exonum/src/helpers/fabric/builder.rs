use std::fmt;

use blockchain::{Service, Blockchain};
use node::{Node, NodeConfig};

use super::internal::{CollectedCommand, Feedback};
use super::clap_backend::ClapBackend;
use super::{Context, ServiceFactory};
use super::details::{Run, Finalize,
                    GenerateNodeConfig, GenerateCommonConfig,
                    GenerateTestnetCommand };
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
                           CollectedCommand::new(Box::new(Run)),
                           CollectedCommand::new(Box::new(GenerateNodeConfig)),
                           CollectedCommand::new(Box::new(GenerateCommonConfig)),
                           CollectedCommand::new(Box::new(Finalize))
                           ],
            service_constructors: Vec::new()
        }
    }

    pub fn with_service<S: ServiceFactory>(mut self) -> NodeBuilder {
        //TODO: take endpoints, etc...

        for ref mut command in self.commands.iter_mut() {
            let name = command.name();
            command.extend(S::command(name))
        }
        self.service_constructors.push(Box::new(S::make_service));
        self
    }

    pub fn parse_cmd(self) -> Option<Node> {
        match ClapBackend::execute(self.commands.as_slice()) {
            Feedback::RunNode(ref ctx) => {
                let db = Run::db_helper(ctx);
                let config: NodeConfig = ctx.get("node_config")
                                            .expect("could not find node_config");
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


impl fmt::Debug for NodeBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NodeBuilder {{ commands: {:?}, services_count: {} }}",
            self.commands,
            self.service_constructors.len()
        )
    }
}
