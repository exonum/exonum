extern crate exonum;
extern crate env_logger;
extern crate clap;
extern crate blockchain_explorer;

use clap::App;

use blockchain_explorer::helpers::GenerateCommand;

fn main() {
    blockchain_explorer::helpers::init_logger().unwrap();

    let app = App::new("Blockchain control utility")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Blockchain control utility")
        .subcommand(GenerateCommand::new());

    let matches = app.get_matches();
    match matches.subcommand() {
        ("generate", Some(matches)) => GenerateCommand::execute(matches),
        _ => unreachable!("Wrong subcommand"),
    }
}