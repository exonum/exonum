// Copyright 2019 The Exonum Team
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
#[macro_use]
extern crate serde_derive;

use exonum::{
    api::backends::actix::AllowOrigin,
    crypto::{PublicKey, PUBLIC_KEY_LENGTH},
    helpers::{
        config::{ConfigFile, ConfigManager},
        fabric::NodeBuilder,
    },
    node::{ConnectInfo, ConnectListConfig, NodeConfig},
};

use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    panic,
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct ConfigSpec {
    expected_root_dir: PathBuf,
    output_root_dir: tempfile::TempDir,
    validators_count: usize,
}

impl ConfigSpec {
    const CONFIG_TESTDATA_FOLDER: &'static str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testdata/config");

    fn new(root_dir: impl AsRef<Path>, validators_count: usize) -> Self {
        Self {
            expected_root_dir: root_dir.as_ref().to_owned(),
            output_root_dir: tempfile::tempdir().unwrap(),
            validators_count,
        }
    }

    fn new_without_pass() -> Self {
        let root_dir = PathBuf::from(Self::CONFIG_TESTDATA_FOLDER).join("without_pass");
        Self::new(root_dir, 4)
    }

    fn new_with_pass() -> Self {
        let root_dir = PathBuf::from(Self::CONFIG_TESTDATA_FOLDER).join("with_pass");
        Self::new(root_dir, 1)
    }

    fn new_more_validators() -> Self {
        let root_dir = PathBuf::from(Self::CONFIG_TESTDATA_FOLDER).join("more_validators");
        Self::new(root_dir, 4)
    }

    fn command(&self, name: &str) -> ArgsBuilder {
        ArgsBuilder {
            args: vec!["exonum-config-test".into(), name.into()],
        }
    }

    fn output_dir(&self) -> PathBuf {
        self.output_root_dir.as_ref().join("cfg")
    }

    fn output_template_file(&self) -> PathBuf {
        self.output_dir().join("template.toml")
    }

    fn output_node_config_dir(&self, index: usize) -> PathBuf {
        self.output_dir().join(index.to_string())
    }

    fn output_sec_config(&self, index: usize) -> PathBuf {
        self.output_node_config_dir(index).join("sec.toml")
    }

    fn output_node_config(&self, index: usize) -> PathBuf {
        self.output_node_config_dir(index).join("node.toml")
    }

    fn expected_dir(&self) -> PathBuf {
        self.expected_root_dir.join("cfg")
    }

    fn expected_template_file(&self) -> PathBuf {
        self.expected_dir().join("template.toml")
    }

    fn expected_node_config_dir(&self, index: usize) -> PathBuf {
        self.expected_dir().join(index.to_string())
    }

    fn expected_node_config_file(&self, index: usize) -> PathBuf {
        self.expected_node_config_dir(index).join("node.toml")
    }

    fn expected_pub_config(&self, index: usize) -> PathBuf {
        self.expected_node_config_dir(index).join("pub.toml")
    }

    fn expected_pub_configs(&self) -> Vec<PathBuf> {
        (0..self.validators_count)
            .map(|i| self.expected_pub_config(i))
            .collect()
    }

    fn expected_sec_config(&self, index: usize) -> PathBuf {
        self.expected_node_config_dir(index).join("sec.toml")
    }
}

#[derive(Debug)]
struct ArgsBuilder {
    args: Vec<OsString>,
}

impl ArgsBuilder {
    fn with_arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    fn with_args(mut self, args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        for arg in args {
            self.args.push(arg.into())
        }
        self
    }

    fn with_named_arg(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.args.push(name.into());
        self.args.push(value.into());
        self
    }

    fn run(self) -> Option<()> {
        log::trace!(
            "-> {}",
            self.args
                .iter()
                .map(|s| s.to_str().unwrap())
                .collect::<Vec<_>>()
                .join(" ")
        );
        if NodeBuilder::new().parse_cmd_string(self.args) {
            None
        } else {
            Some(())
        }
    }
}

fn touch(path: impl AsRef<Path>) {
    OpenOptions::new()
        .create(true)
        .write(true)
        .open(path)
        .unwrap();
}

fn load_node_config(path: impl AsRef<Path>) -> NodeConfig<PathBuf> {
    ConfigFile::load(path).expect("Can't load node config file")
}

fn assert_config_files_eq(path_1: impl AsRef<Path>, path_2: impl AsRef<Path>) {
    let cfg_1: toml::Value = ConfigFile::load(&path_1).unwrap();
    let cfg_2: toml::Value = ConfigFile::load(&path_2).unwrap();
    assert_eq!(
        cfg_1,
        cfg_2,
        "file {:?} doesn't match with {:?}",
        path_1.as_ref(),
        path_2.as_ref()
    );
}

// Special case for NodeConfig because it uses absolute paths for secret key files.
fn assert_node_config_files_eq(actual: impl AsRef<Path>, expected: impl AsRef<Path>) {
    let (actual, expected) = (actual.as_ref(), expected.as_ref());

    let config_dir = expected.parent().unwrap();
    let actual = load_node_config(actual);
    let mut expected = load_node_config(expected);
    expected.service_secret_key = config_dir.join(&expected.service_secret_key);
    expected.consensus_secret_key = config_dir.join(&expected.consensus_secret_key);

    assert_eq!(actual, expected);
}

#[test]
fn test_allow_origin_toml() {
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
fn test_generate_template() {
    let env = ConfigSpec::new_without_pass();
    let output_template_file = env.output_template_file();
    env.command("generate-template")
        .with_arg(&output_template_file)
        .with_named_arg("--validators-count", env.validators_count.to_string())
        .run()
        .unwrap();
    assert_config_files_eq(&output_template_file, env.expected_template_file());
}

#[test]
fn test_generate_config_key_files() {
    let env = ConfigSpec::new_without_pass();
    env.command("generate-config")
        .with_arg(&env.expected_template_file())
        .with_arg(&env.output_node_config_dir(0))
        .with_named_arg("-a", "0.0.0.0:8000")
        .with_arg("--no-password")
        .run()
        .unwrap();

    let sec_cfg: toml::Value = ConfigFile::load(&env.output_sec_config(0)).unwrap();
    assert_eq!(sec_cfg["consensus_secret_key"], "consensus.key.toml".into());
    assert_eq!(sec_cfg["service_secret_key"], "service.key.toml".into());
}

#[test]
fn test_generate_config_ipv4() {
    let env = ConfigSpec::new_without_pass();
    env.command("generate-config")
        .with_arg(&env.expected_template_file())
        .with_arg(&env.output_node_config_dir(0))
        .with_named_arg("-a", "127.0.0.1")
        .with_arg("--no-password")
        .run()
        .unwrap()
}

#[test]
fn test_generate_config_ipv6() {
    let env = ConfigSpec::new_without_pass();
    env.command("generate-config")
        .with_arg(&env.expected_template_file())
        .with_arg(&env.output_node_config_dir(0))
        .with_named_arg("-a", "::1")
        .with_arg("--no-password")
        .run()
        .unwrap()
}

#[test]
fn test_finalize_run_without_pass() {
    let env = ConfigSpec::new_without_pass();

    env::set_var("EXONUM_CONSENSUS_PASS", "");
    env::set_var("EXONUM_SERVICE_PASS", "");
    for i in 0..env.validators_count {
        let node_config = env.output_node_config(i);
        env.command("finalize")
            .with_arg(env.expected_sec_config(i))
            .with_arg(&node_config)
            .with_arg("--public-configs")
            .with_args(env.expected_pub_configs())
            .run()
            .unwrap();
        assert_node_config_files_eq(&node_config, env.expected_node_config_file(i));

        let feedback = env
            .command("run")
            .with_named_arg("-c", &node_config)
            .with_named_arg("-d", env.output_dir().join("foo"))
            .with_named_arg("--service-key-pass", "env")
            .with_named_arg("--consensus-key-pass", "env")
            .run();
        assert!(feedback.is_none());
    }
}

#[test]
fn test_finalize_run_with_pass() {
    let env = ConfigSpec::new_with_pass();

    env::set_var("EXONUM_CONSENSUS_PASS", "some passphrase");
    env::set_var("EXONUM_SERVICE_PASS", "another passphrase");
    let node_config = env.output_node_config(0);
    env.command("finalize")
        .with_arg(env.expected_sec_config(0))
        .with_arg(&node_config)
        .with_arg("--public-configs")
        .with_args(env.expected_pub_configs())
        .run()
        .unwrap();
    assert_node_config_files_eq(&node_config, env.expected_node_config_file(0));

    let feedback = env
        .command("run")
        .with_named_arg("-c", &node_config)
        .with_named_arg("-d", env.output_dir().join("foo"))
        .with_named_arg("--service-key-pass", "env")
        .with_named_arg("--consensus-key-pass", "env")
        .run();
    assert!(feedback.is_none());
}

#[test]
#[should_panic(
    expected = "The number of validators configs does not match the number of validators keys."
)]
fn test_less_validators_count() {
    let env = ConfigSpec::new_without_pass();

    env::set_var("EXONUM_CONSENSUS_PASS", "");
    env::set_var("EXONUM_SERVICE_PASS", "");

    let node_config = env.output_node_config(0);
    env.command("finalize")
        .with_arg(env.expected_sec_config(0))
        .with_arg(&node_config)
        .with_arg("--public-configs")
        .with_args(env.expected_pub_configs().into_iter())
        .run()
        .unwrap();
}

#[test]
#[should_panic(
    expected = "The number of validators configs does not match the number of validators keys."
)]
fn test_more_validators_count() {
    let env = ConfigSpec::new_more_validators();

    env::set_var("EXONUM_CONSENSUS_PASS", "");
    env::set_var("EXONUM_SERVICE_PASS", "");

    let node_config = env.output_node_config(0);
    env.command("finalize")
        .with_arg(env.expected_sec_config(0))
        .with_arg(&node_config)
        .with_arg("--public-configs")
        .with_args(env.expected_pub_configs())
        .run()
        .unwrap();
}

#[test]
fn test_run_dev() {
    let env = ConfigSpec::new_without_pass();

    let artifacts_dir = env.output_dir().join("artifacts");
    // Mocks existence of old DB files that are supposed to be cleaned up.
    let db_dir = artifacts_dir.join("db");
    fs::create_dir_all(&db_dir).unwrap();
    let old_db_file = db_dir.join("content.foo");
    touch(&old_db_file);
    // Checks run-dev command.
    let feedback = env
        .command("run-dev")
        .with_arg("-a")
        .with_arg(&artifacts_dir)
        .run();
    assert!(feedback.is_none());
    // Tests cleaning up.
    assert!(!old_db_file.exists());
}

#[test]
fn test_update_config() {
    let env = ConfigSpec::new_without_pass();
    let config_path = env.output_dir().join("node.toml");
    fs::create_dir(&config_path.parent().unwrap()).unwrap();
    fs::copy(&env.expected_node_config_file(0), &config_path).unwrap();

    // Test config update.
    let peer = ConnectInfo {
        address: "0.0.0.1:8080".to_owned(),
        public_key: PublicKey::new([1; PUBLIC_KEY_LENGTH]),
    };

    let connect_list = ConnectListConfig { peers: vec![peer] };

    ConfigManager::update_connect_list(connect_list.clone(), &config_path)
        .expect("Unable to update connect list");
    let config = load_node_config(&config_path);

    let new_connect_list = config.connect_list;
    assert_eq!(new_connect_list.peers, connect_list.peers);
}
