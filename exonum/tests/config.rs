// Copyright 2018 The Exonum Team
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
extern crate exonum;
#[macro_use]
extern crate pretty_assertions;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use exonum::{
    api::backends::actix::AllowOrigin, crypto::PublicKey, encoding::serialize::FromHex,
    helpers::{
        config::{ConfigFile, ConfigManager}, fabric::NodeBuilder,
    },
    node::{ConnectInfo, ConnectListConfig, NodeConfig},
};
use toml::Value;

use std::{
    ffi::OsString, fs, fs::{File, OpenOptions}, io::{Read, Write}, net::SocketAddr, panic,
    path::Path, str::FromStr,
};

const CONFIG_TMP_FOLDER: &str = "/tmp/";
const CONFIG_TESTDATA_FOLDER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testdata/config/");

const GENERATED_TEMPLATE: &str = "template.toml";

const SEC_CONFIG: [&str; 4] = [
    "config0_sec.toml",
    "config1_sec.toml",
    "config2_sec.toml",
    "config3_sec.toml",
];

const PUB_CONFIG: [&str; 4] = [
    "config0_pub.toml",
    "config1_pub.toml",
    "config2_pub.toml",
    "config3_pub.toml",
];

fn full_tmp_folder(folder: &str) -> String {
    format!("{}exonum-test-{}/", CONFIG_TMP_FOLDER, folder)
}

fn full_tmp_name(filename: &str, folder: &str) -> String {
    format!("{}{}", full_tmp_folder(folder), filename)
}

fn full_testdata_name(filename: &str) -> String {
    format!("{}{}", CONFIG_TESTDATA_FOLDER, filename)
}

fn touch(path: &str) {
    OpenOptions::new()
        .create(true)
        .write(true)
        .open(path)
        .unwrap();
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
        "--validators-count",
        "1",
    ]));
}

#[derive(Debug, Clone, Copy)]
enum IpMode {
    V4,
    V6,
}

fn generate_config(folder: &str, i: usize, mode: IpMode) {
    let ip = match mode {
        IpMode::V4 => "127.0.0.1",
        IpMode::V6 => "::1",
    };
    assert!(!default_run_with_matches(vec![
        "exonum-config-test",
        "generate-config",
        &full_testdata_name(GENERATED_TEMPLATE),
        &full_tmp_name(PUB_CONFIG[i], folder),
        &full_tmp_name(SEC_CONFIG[i], folder),
        "-a",
        ip,
    ]));
}

fn finalize_config(folder: &str, config: &str, i: usize, count: usize) {
    let mut variables = vec![
        "exonum-config-test".to_owned(),
        "finalize".to_owned(),
        full_testdata_name(SEC_CONFIG[i]),
        full_tmp_name(config, folder),
        "-p".to_owned(),
    ];

    fs::create_dir_all(full_tmp_name("", folder)).expect("Can't create temp folder");

    for &conf in PUB_CONFIG.iter().take(count) {
        override_validators_count(conf, count, folder);
        variables.push(full_tmp_name(conf, folder));
    }
    assert!(!default_run_with_matches(variables));
}

fn override_validators_count(config: &str, n: usize, folder: &str) {
    let res = {
        let mut contents = String::new();
        let mut file = File::open(full_testdata_name(config)).unwrap();
        file.read_to_string(&mut contents)
            .expect("Read from config file failed");

        let mut value = contents.as_str().parse::<Value>().unwrap();
        {
            let mut count = value
                .get_mut("common")
                .unwrap()
                .get_mut("general_config")
                .unwrap()
                .as_table_mut()
                .unwrap();

            count.insert("validators_count".into(), Value::from(n as u8));
        }

        toml::to_string(&value).unwrap()
    };

    File::create(full_tmp_name(config, folder))
        .unwrap()
        .write_all(res.as_bytes())
        .expect("Create temp config file is failed");
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

fn run_dev(folder: &str) {
    assert!(default_run_with_matches(vec![
        "exonum-config-test",
        "run-dev",
        "-a",
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

fn test_generate_config(mode: IpMode) {
    // Important because tests run in parallel, folder names should be different.
    let command = match mode {
        IpMode::V4 => "generate-config-ipv4",
        IpMode::V6 => "generate-config-ipv6",
    };

    let result = panic::catch_unwind(|| {
        for i in 0..PUB_CONFIG.len() {
            generate_config(command, i, mode);
        }
    });

    fs::remove_dir_all(full_tmp_folder(command)).unwrap();

    if let Err(err) = result {
        panic::resume_unwind(err);
    }
}

#[test]
fn test_generate_config_ipv4() {
    test_generate_config(IpMode::V4);
}

#[test]
fn test_generate_config_ipv6() {
    test_generate_config(IpMode::V6);
}

#[test]
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

#[test]
fn test_run_dev() {
    let artifacts_dir = ".exonum";
    let db_dir = format!("{}/{}", artifacts_dir, "db");
    let full_db_dir = full_tmp_folder(&db_dir);

    // Mock existence of old DB files that are supposed to be cleaned up.
    fs::create_dir_all(Path::new(&full_db_dir)).expect("Expected db temp folder to be created.");
    let old_db_file = full_tmp_name("1", &db_dir);
    touch(&old_db_file);

    let result = panic::catch_unwind(|| {
        run_dev(artifacts_dir);

        // Test cleaning up.
        assert!(!Path::new(&old_db_file).exists());
    });

    fs::remove_dir_all(full_tmp_folder(artifacts_dir)).unwrap();

    if let Err(err) = result {
        panic::resume_unwind(err);
    }
}

#[test]
fn allow_origin_toml() {
    fn check(text: &str, allow_origin: AllowOrigin) {
        #[derive(Serialize, Deserialize)]
        struct Config {
            allow_origin: AllowOrigin,
        }
        let config_toml = format!("allow_origin = {}\n", text);
        let config: Config = ::toml::from_str(&config_toml).unwrap();
        assert_eq!(config.allow_origin, allow_origin);
        assert_eq!(::toml::to_string(&config).unwrap(), config_toml);
    }

    check(r#""*""#, AllowOrigin::Any);
    check(
        r#""http://example.com""#,
        AllowOrigin::Whitelist(vec!["http://example.com".to_string()]),
    );
    check(
        r#"["http://a.org", "http://b.org"]"#,
        AllowOrigin::Whitelist(vec!["http://a.org".to_string(), "http://b.org".to_string()]),
    );
}

#[test]
fn test_update_config() {
    const TEST_DIR: &str = "config-update";
    const TEST_CONFIG_FILE: &str = "config01.toml";

    let full_test_dir = full_tmp_folder(TEST_DIR);
    fs::create_dir_all(Path::new(&full_test_dir))
        .expect("Expected test temp folder to be created.");

    // Copy test config to the separate file.
    let config_path = full_tmp_name(TEST_CONFIG_FILE, TEST_DIR);
    let testdata_path = full_testdata_name(TEST_CONFIG_FILE);
    fs::copy(testdata_path, config_path.clone()).unwrap();

    // Test config update.
    let peer = ConnectInfo {
        address: SocketAddr::from_str("0.0.0.1:8080").unwrap(),
        public_key: PublicKey::new([1; 32]),
    };

    let connect_list = ConnectListConfig { peers: vec![peer] };

    ConfigManager::update_connect_list(connect_list.clone(), &config_path)
        .expect("Unable to update connect list");
    let config: NodeConfig =
        ConfigFile::load(config_path.clone()).expect("Can't load node config file");

    let new_connect_list = config.connect_list;
    assert_eq!(new_connect_list.peers, connect_list.peers);

    // Cleanup.
    fs::remove_dir_all(Path::new(&full_test_dir)).unwrap();
}

#[test]
fn test_domain_name_peer() {
    const TEST_CONFIG_FILE: &str = "config_domain.toml";
    let testdata_path = full_testdata_name(TEST_CONFIG_FILE);
    let config: ConnectListConfig =
        ConfigFile::load(testdata_path).expect("Can't load node config file");

    let connect_list = ConnectListConfig {
        peers: vec![
            ConnectInfo {
                address: "127.0.0.1:6333".parse().unwrap(),
                public_key: PublicKey::from_hex(
                    "16ef83ca4b231404daec6d07b24beb84d89c25944285d2e32a2dcf8f0f3eda72",
                ).unwrap(),
            },
            ConnectInfo {
                address: "127.0.0.1:6333".parse().unwrap(),
                public_key: PublicKey::from_hex(
                    "648e98a2405a40325d946bf8de6937795fe5c22ab095bca765a8b218e49ff5a3",
                ).unwrap(),
            },
            ConnectInfo {
                address: "94.130.16.228:6333".parse().unwrap(),
                public_key: PublicKey::from_hex(
                    "924625eb77b9ad21e76713e7ada715945fbf0a926698832e121484c797fcc58e",
                ).unwrap(),
            },
        ],
    };

    assert_eq!(config.peers, connect_list.peers);
}
