use blockchain::{Service, Blockchain};
use node::{Node, NodeConfig};

use super::internal::{CollectedCommand, Feedback};
use super::clap_backend::ClapBackend;
use super::{Context, ServiceFactory};
use super::details::{GenerateTestnetCommand, RunCommand, AddValidatorCommand, 
                     KeyGeneratorCommand, GenerateTemplateCommand, InitCommand};
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
                           CollectedCommand::new(Box::new(AddValidatorCommand)),
                           CollectedCommand::new(Box::new(KeyGeneratorCommand)),
                           CollectedCommand::new(Box::new(GenerateTemplateCommand)),
                           CollectedCommand::new(Box::new(InitCommand))
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

    pub fn parse_cmd(self) -> Option<Node> {
        match ClapBackend::execute(self.commands.as_slice()) {
            Feedback::RunNode(ref ctx) => {
                let db = RunCommand::db_helper(ctx);
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
