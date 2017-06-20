/// `NodeBuilder` is a high level object,
/// usable for fast prototyping and creating app from services list.

use clap;
use toml::Value;
use serde::Deserialize;

use std::iter;
use std::rc::Rc;
use std::path::Path;
use std::error::Error;
use std::net::SocketAddr;
use std::collections::BTreeMap;

use storage::Storage;
use blockchain::{Service, Blockchain};
use node::{Node, NodeConfig};
use config::ConfigFile;

type CommandName = &'static str;

struct NodeBuilder {
    commands: Vec<CollectedCommand>,
    service_constructors: Vec<Box<FnMut(&Context) -> Box<Service>>>,
}

impl NodeBuilder {

    pub fn new() -> NodeBuilder {
        NodeBuilder {
            commands: Vec::new(),
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

        let path = ctx.get::<String>("leveldb_path")
                      .expect("leveldb_path not found.");
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
        let path = ctx.get::<String>("node_config_path")
                      .expect("node_config_path not found.");
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
        ctx.get::<String>("PUBLIC_API_ADDRESS")
            .map(|s|
                s.parse()
                 .expect("Public api address has incorrect format"))
    }

    pub fn private_api_address(ctx: &Context) -> Option<SocketAddr> {
        ctx.get::<String>("PRIVATE_API_ADDRESS")
            .map(|s|
                s.parse()
                 .expect("Public api address has incorrect format"))
    }

    pub fn create_node(self) -> Option<Node> {
        match ClapBackend::execute(self.commands.as_slice()) {
            Feedback::RunNode(ref ctx) => {
                let db = Self::db_helper(ctx);
                let config = Self::node_config(ctx);
                let services: Vec<Box<Service>> = self.service_constructors
                                                      .into_iter()
                                                      .map(|mut constructor| constructor(ctx))
                                                      .collect();
                let blockchain = Blockchain::new(db, services);
                let mut node = Node::new(blockchain, config);
                Some(node)
            }
            _ => None
        }
    }

    pub fn run(self) {
        self.create_node()
            .expect("Expected run command")
            .run()
            .expect("Node return error")
    }
}

pub struct Argument {
    pub short_name: String,
    pub long_name: String,
    pub required: bool,
    pub help: String,
}

impl Argument {
    fn into_clap(&self) -> clap::Arg {
        clap::Arg::with_name(&self.long_name)
            .short(&self.short_name)
            .long(&self.long_name)
            .help(&self.help)
            .required(self.required)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Context {
    values: BTreeMap<String, Value>
}

impl Context {

    fn new_from_args(args: &Vec<Argument>, matches: &clap::ArgMatches) -> Context {
        let mut context = Context::default();
        for arg in args {
            if let Some(value) = matches.value_of(&arg.long_name) {
                if context.values.insert(arg.long_name.clone(), value.to_string().into()).is_some() {
                    // TODO: replace by `unreachable!` 
                    // after making it unreachable ;)
                    panic!("Found args dupplicate, in args list.");
                }
                else {
                    if arg.required {
                        panic!("Argument {} not found.", arg.long_name)
                    }
                }
            }
        }
        context
    }

    fn get<'de, T: Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.values.get(key)
                   .expect("Expected Some in getting context.")
                   .clone()
                   .try_into()
                   .ok()
    }
}

pub trait CommandExtension {
    fn args(&self) -> Vec<Argument>;
    fn execute(&self, context: Context) -> Result<Context, Box<Error>>;
}

//TODO: we could extend current feedback
/// Used to take some additional information from executed command
pub enum Feedback {
    RunNode(Context),
    None,
}

pub trait Command {
    fn args(&self) -> Vec<Argument>;
    fn name(&self) -> CommandName ;
    fn about(&self) -> &str;
    fn execute(&self, 
               context: Context,
               extension: &Fn(Context) -> Context) -> Feedback;
}

struct CollectedCommand {
    command: Box<Command>,
    args: Vec<Argument>,
    exts: Vec<Box<CommandExtension>>
}

impl CollectedCommand {
    fn new(command: Box<Command>) -> CollectedCommand {
        CollectedCommand {
            args: command.args(),
            command: command,
            exts: Vec::new()
        }
    }

    fn args(&self) -> &Vec<Argument> {
        &self.args
    }

    fn name(&self) -> CommandName {
        self.command.name()
    }

    fn about(&self) -> &str {
        self.command.about()
    }

    fn extend(&mut self, extender: Box<CommandExtension>) {
        let args = extender.args();
        self.args.extend(args.into_iter());
        self.exts.push(extender);
    }

    fn execute(&self, context: Context) -> Feedback {
        self.command.execute(context, &|context| {
            let mut new_context = context.clone();
            for ext in &self.exts {
                let local_context = context.clone();
                let out_context = ext.execute(local_context)
                                     .expect("Could not execute extension.");
                // TODO: check duplicates
                new_context.values.extend(out_context.values.into_iter());
            };
            new_context
        })
    }
}

pub trait ServiceFactory: 'static {
    //TODO: we could move 
    // `service_name` and `service_id` from `Service` trait into this one
    //fn name() -> &'static str;
    /// return `CommandExtension` for specific `CommandName`
    fn command(command: CommandName) -> Box<CommandExtension>;
    /// create new service, from context, returned by `run` command.
    fn make_service(self, run_context: &Context) -> Box<Service>;
}

struct ClapBackend;

impl ClapBackend {

    fn execute(commands: &[CollectedCommand]) -> Feedback {
        let app = 
        clap::App::new("Exonum application based on fabric configuration.")
                .version(env!("CARGO_PKG_VERSION"))
                .author("Vladimir M. <vladimir.motylenko@xdev.re>")
                .about("Exonum application based on fabric configuration.");

        let subcommands: Vec<_> = commands.iter().map(|command|
            ClapBackend::into_subcommand(command)
        ).collect();

        let matches = app.subcommands(subcommands.into_iter()).get_matches();

        

        let subcommand = matches.subcommand();
        for command in commands {
            if command.name() == subcommand.0 {
                return command.execute(
                            Context::new_from_args(
                                command.args(),
                                subcommand.1.expect("Arguments not found.")
                            ))
            }
        }

        panic!("Subcommand not found");
    }

    fn into_subcommand<'a>(command: &'a CollectedCommand) -> clap::App<'a, 'a>{
        let command_args: Vec<_> = command.args()
                                  .iter()
                                  .map(|arg| arg.into_clap())
                                  .collect();

        let mut subcommand = clap::SubCommand::with_name(command.name())
            .about(command.about());

        subcommand = subcommand.args(&command_args);

        subcommand
    }
}