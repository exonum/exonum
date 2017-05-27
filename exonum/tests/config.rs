// This is regression test for exonum configuration.
#[macro_use] extern crate pretty_assertions;
extern crate toml;
extern crate exonum;
extern crate clap;
extern crate lazy_static;

use std::ffi::OsString;
use std::fs::File;
use std::fs;
use std::io::Read;
use std::path::Path;
use clap::{App, Result};

use exonum::helpers::clap::{GenerateTemplateCommand, GenerateTestnetCommand,
                                AddValidatorCommand, InitCommand };

const CONFIG_TMP_FOLDER: &'static str = "/tmp/";
const CONFIG_TESTDATA_FOLDER: &'static str = 
                    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testdata/config/");

const GENERATED_TEMPLATES: [&'static str; 5] = ["template.toml", "template1.toml", 
                                                "template2.toml", "template3.toml",
                                                "template_full.toml", ];

const KEYCHAINS: [&'static str; 4] = ["keychain1.toml","keychain2.toml",
                                        "keychain3.toml","keychain4.toml"];

const OUT_CONFIGS : [&'static str; 4] = ["config1.toml","config2.toml",
                                        "config3.toml","config4.toml"];

const PUB_KEYS: [&'static str; 4] = ["keychain1.pub","keychain2.pub",
                                        "keychain3.pub","keychain4.pub"];

const START_TEMPLATE: &'static str = GENERATED_TEMPLATES[0];
const FULL_TEMPLATE: &'static str = GENERATED_TEMPLATES[4];

fn full_tmp_folder(folder: &str) -> String {
    format!("{}exonum-test-{}/", CONFIG_TMP_FOLDER, folder)
}

fn full_tmp_name(filename: &str, folder: &str) -> String {
    format!("{}{}", full_tmp_folder(folder), filename)
}

fn full_testdata_name(filename: &str) -> String {
    format!("{}{}", CONFIG_TESTDATA_FOLDER, filename)
}

fn compare_files(filename: &str, folder: &str) {
    let source = full_testdata_name(filename);
    let destination = full_tmp_name(filename, folder);

    let mut source = File::open(source).unwrap();
    let mut destination = File::open(destination).unwrap();
    
    let mut source_buffer = String::new();
    let mut destination_buffer = String::new();
    
    let len = source.read_to_string(&mut source_buffer).unwrap();
    destination.read_to_string(&mut destination_buffer).unwrap();
    
    assert!(len > 0);
    let source_toml: toml::Value = toml::de::from_str(&source_buffer).unwrap();
    let destination_toml: toml::Value = toml::de::from_str(&destination_buffer).unwrap();
    assert_eq!(source_toml, destination_toml);
}

fn default_run_with_matches<I, T>(iter: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone, 
{
    let app = App::new("Test template generate")
                    .subcommand(GenerateTemplateCommand::new())
                    .subcommand(GenerateTestnetCommand::new())
                    .subcommand(AddValidatorCommand::new())
                    .subcommand(InitCommand::new());

    let matches = app.get_matches_from_safe(iter)?;

    match matches.subcommand() {
        ("generate-template", Some(matches)) => GenerateTemplateCommand::execute(matches, None),
        ("add-validator", Some(matches)) => AddValidatorCommand::execute(matches, None),
        ("generate", Some(matches)) => GenerateTestnetCommand::execute(matches),
        ("init", Some(matches)) => InitCommand::execute(matches, None),
        _ => panic!("Wrong subcommand"),
    };
    Ok(())
}

fn generate_template(folder: &str) {
    default_run_with_matches(vec!["exonum-config-test", "generate-template", "4",
                                    &full_tmp_name(START_TEMPLATE, folder)]).unwrap();
    
}

fn add_validator(folder: &str, i: usize) {
    default_run_with_matches(vec!["exonum-config-test", "add-validator", 
                                    &full_tmp_name(GENERATED_TEMPLATES[i + 1], folder),
                                    &full_testdata_name(PUB_KEYS[i]),
                                    "-a",
                                    "127.0.0.1"]
                                    ).unwrap();
}

fn init_validator(config: &str, folder: &str, i: usize) {
    default_run_with_matches(vec!["exonum-config-test", "init", 
                                    &full_testdata_name(config),
                                    &full_testdata_name(KEYCHAINS[i]),
                                    &full_tmp_name(OUT_CONFIGS[i], folder)]
                                    ).unwrap();  
}

fn deploy_file(file:&str, folder:&str) {
    let dest = full_tmp_name(file, folder);
    let path: &Path = dest.as_ref();
    if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).unwrap();
    }
    fs::copy(&full_testdata_name(file), 
                path).unwrap();
}

#[test]
fn test_generate_template() {
    let command = "generate-template";
    generate_template(command);
    compare_files(START_TEMPLATE, command);
    fs::remove_dir_all(full_tmp_folder(command)).unwrap();
}

#[test]
#[cfg_attr(feature="cargo-clippy", allow(needless_range_loop))]
fn test_add_validators_full_template() {
    let command = "add-validator";
    deploy_file(START_TEMPLATE, command);
    for i in 0..KEYCHAINS.len() {
        fs::rename(&full_tmp_name(GENERATED_TEMPLATES[i], command),
                         &full_tmp_name(GENERATED_TEMPLATES[i + 1], command)).unwrap();
        add_validator(command, i);
        compare_files(GENERATED_TEMPLATES[i + 1], command);
    }
    fs::remove_dir_all(full_tmp_folder(command)).unwrap();
}

#[test]
#[cfg_attr(feature="cargo-clippy", allow(needless_range_loop))]
fn test_generate_full_config() {
    let command = "init";
    for i in 0..KEYCHAINS.len() {
        init_validator(FULL_TEMPLATE, command, i);
        compare_files(OUT_CONFIGS[i], command);
    }
    fs::remove_dir_all(full_tmp_folder(command)).unwrap();
}
