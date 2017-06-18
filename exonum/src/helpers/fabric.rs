/// `NodeBuilder` is a high level object,
/// usable for fast prototyping and creating app from services list.

use toml::Value;
use std::iter;

type CommandId = &'static str;

static CORE_COMMANDS_LIST: [CommandId; 5] = [
    "run",
    "generate-testnet",
    "generate-template",
    "add-validator",
    "init",
];

type ExtendedCommands = BTreeMap<Command, ExtendedCommand>;

struct NodeBuilder {
    commands: ExtendedCommands,
    services: Box<Service>
}

impl NodeBuilder {

    fn new() -> NodeBuilder
    {
        let commands = CORE_COMMANDS_LIST
                        .iter()
                        .zip(iter::repeat(Vec::new()))
                        .collect()
        NodeBuilder {
            commands
        }
    }

    fn with_service<S: ServiceFactory>(mut self, service: S) -> NodeBuilder {
        //TODO: take endpoints, etc...
        for command in self.commands.iter_mut() {
            command.push(service.command(command));
        }
        self
    }

    fn to_node(self) -> Node {
        self.parse_cmd()
    }

    fn run(self) {
        self.to_node().run()
    }
}

struct Argument {
    pub short_name: String,
    pub long_name: String,
    pub required: bool,
    pub help: String,
}

impl Argument {
    fn into_clap(&self) -> clap::Arg {
        Arg::with_name(&self.long_name)
            .short(&self.short_name)
            .long(&self.long_name)
            .about(&self.help)
            .required(self.required)
    }
}

struct Context {
    values: BTreeMap<String, Value>
}

trait CommandExtender {
    fn args(&self ) -> Vec<Argument>;
    fn execute(&self, context: Context) -> Result<Context, Box<Error>>;
}

/*
#[Debug, Copy, Clone, Eq, PartialEq]
pub struct ExtendedCommand {
    name: &'static str,
    services: Vec<Box<CommandExtender>>
}
*/

trait ExtendedCommand {

    fn id(&self) -> CommandId;
    fn name(&self) -> &str {
        self.id()
    }
    
    fn extend(&mut self, extender: Box<CommandExtender>);

    fn about(&self) -> &str;

    pub fn execute(&self);
}


trait ServiceFactory {
    //\TODO we could move 
    // `service_name` and `service_id` from `Service` trait into this one
    //fn name() -> &'static str;
    /// return `CommandExtender` for specific `CommandId`
    fn command(command: CommandId) -> Box<CommandExtender>;
    /// create new service, from context, returned by `run` command.
    fn make_service(self, run_context: &Context) -> Box<Service>;
}


struct ClapBacked;

impl ClapBackend {
    fn execute(commands: &ExtendedCommands) {
        let mut app = App::new(command.about())
                .version(env!("CARGO_PKG_VERSION"))
                .author("Vladimir M. <vladimir.motylenko@xdev.re>")
                .about(command.about());

        for c in commands.iter() {
             let app = app.subcommand(ClapBackend::into_subcommand(c));
        }

        let matches = app.get_matches();
        let command = commands.get(matches.subcommand()); 
        command.expect("Subcommand not found").execute();
    }

    fn into_subcommand<'a, 'a>(command: &'a ExtendedCommand) -> App<'a, 'a>{
        let command_args = command.args()
                                  .iter()
                                  .map(|command|command.into_clap());
        let mut subcommand = SubCommand::with_name(command.name())
            .about(command.about());
        for command in command_args {
            subcommand = subcommand.arg(command);
        }

        subcommand
    }
}