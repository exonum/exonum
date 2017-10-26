// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// This is a regression test for exonum configuration.
#[macro_use]
extern crate pretty_assertions;
extern crate toml;
extern crate exonum;

use std::ffi::OsString;
use std::fs::File;
use std::fs;
use std::panic;
use std::io::Read;

use exonum::helpers::fabric::NodeBuilder;

const CONFIG_TMP_FOLDER: &'static str = "/tmp/";
const CONFIG_TESTDATA_FOLDER: &'static str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testdata/config/");

const GENERATED_TEMPLATE: &'static str = "template.toml";

const SEC_CONFIG: [&'static str; 4] =
    ["config0_sec.toml", "config1_sec.toml", "config2_sec.toml", "config3_sec.toml"];

const PUB_CONFIG: [&'static str; 4] =
    ["config0_pub.toml", "config1_pub.toml", "config2_pub.toml", "config3_pub.toml"];

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

fn default_run_with_matches<I, T>(iter: I) -> bool
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let builder = NodeBuilder::new();
    builder.parse_cmd_string(iter)
}

fn generate_template(folder: &str) {
    assert!(!default_run_with_matches(vec![
        "exonum-config-test",
        "generate-template",
        &full_tmp_name(GENERATED_TEMPLATE, folder),
    ]));

}

fn generate_config(folder: &str, i: usize) {
    assert!(!default_run_with_matches(vec![
        "exonum-config-test",
        "generate-config",
        &full_testdata_name(GENERATED_TEMPLATE),
        &full_tmp_name(PUB_CONFIG[i], folder),
        &full_tmp_name(SEC_CONFIG[i], folder),
        "-a",
        "127.0.0.1",
    ]));
}

#[cfg_attr(feature = "cargo-clippy", allow(needless_range_loop))]
fn finalize_config(folder: &str, config: &str, i: usize, count: usize) {

    let mut variables = vec![
        "exonum-config-test".to_owned(),
        "finalize".to_owned(),
        full_testdata_name(SEC_CONFIG[i]),
        full_tmp_name(config, folder),
        "-p".to_owned(),
    ];
    for n in 0..count {
        variables.push(full_testdata_name(PUB_CONFIG[n]));
    }
    println!("{:?}", variables);
    assert!(!default_run_with_matches(variables));
}

fn run_node(config: &str, folder: &str) {
    assert!(default_run_with_matches(vec![
        "exonum-config-test",
        "run",
        "-c",
        &full_testdata_name(config),
        "-d",
        &full_tmp_folder(folder),
    ]));
}

#[test]
fn test_generate_template() {
    let command = "generate-template";

    let result = panic::catch_unwind(|| {
        generate_template(command);
        compare_files(GENERATED_TEMPLATE, command);
    });

    fs::remove_dir_all(full_tmp_folder(command)).unwrap();

    if let Err(err) = result {
        panic::resume_unwind(err);
    }
}

#[test]
#[cfg_attr(feature = "cargo-clippy", allow(needless_range_loop))]
fn test_generate_config() {
    let command = "generate-config";

    let result = panic::catch_unwind(|| for i in 0..PUB_CONFIG.len() {
        generate_config(command, i);
    });

    fs::remove_dir_all(full_tmp_folder(command)).unwrap();

    if let Err(err) = result {
        panic::resume_unwind(err);
    }
}

#[test]
#[cfg_attr(feature = "cargo-clippy", allow(needless_range_loop))]
fn test_generate_full_config_run() {
    let command = "finalize";
    let result = panic::catch_unwind(|| {
        for i in 0..PUB_CONFIG.len() {
            for n in 0..PUB_CONFIG.len() + 1 {
                println!("{} {}", i, n);
                let config = format!("config{}{}.toml", i, n);
                let result = panic::catch_unwind(|| {
                    finalize_config(command, &config, i, n);
                    compare_files(&config, command);
                    run_node(&config, command);
                });

                // if we trying to create config,
                // without our config, this is a problem
                if n <= i || n == 0 {
                    assert!(result.is_err());
                } else {
                    assert!(result.is_ok());
                }
            }

        }
    });

    fs::remove_dir_all(full_tmp_folder(command)).unwrap();

    if let Err(err) = result {
        panic::resume_unwind(err);
    }
}
