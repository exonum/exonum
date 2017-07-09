use std::fmt;
use std::ffi::OsString;

use blockchain::{Service, Blockchain};
use node::{Node, NodeConfig};

use super::internal::{CollectedCommand, Feedback};
use super::clap_backend::ClapBackend;
use super::{Context, ServiceFactory};
use super::details::{Run, Finalize,
                    GenerateNodeConfig, GenerateCommonConfig,
                    GenerateTestnet };
/// `NodeBuilder` is a high level object,
/// usable for fast prototyping and creating app from services list.
#[derive(Default)]
pub struct NodeBuilder {
    commands: Vec<CollectedCommand>,
    service_constructors: Vec<Box<FnMut(&Context) -> Box<Service>>>,
}

impl NodeBuilder {

    /// creates new empty `NodeBuilder`
    pub fn new() -> NodeBuilder {
        NodeBuilder {
            commands: vec![CollectedCommand::new(Box::new(GenerateTestnet)),
                           CollectedCommand::new(Box::new(Run)),
                           CollectedCommand::new(Box::new(GenerateNodeConfig)),
                           CollectedCommand::new(Box::new(GenerateCommonConfig)),
                           CollectedCommand::new(Box::new(Finalize))
                           ],
            service_constructors: Vec::new()
        }
    }

    /// append service to `NodeBuilder` context
    pub fn with_service<S: ServiceFactory>(mut self) -> NodeBuilder {
        //TODO: take endpoints, etc...

        for ref mut command in &mut self.commands {
            let name = command.name();
            command.extend(S::command(name))
        }
        self.service_constructors.push(Box::new(S::make_service));
        self
    }

    #[doc(hiden)]
    pub fn parse_cmd_string<I, T>(self, cmd_line: I) -> bool 
    where I: IntoIterator<Item=T>, T: Into<OsString> + Clone
    {
        ClapBackend::execute_cmd_string(self.commands.as_slice(), cmd_line) 
            != Feedback::None

    }

    /// Parse cmd args, return `Node`, if run command found
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

    /// Run application
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
