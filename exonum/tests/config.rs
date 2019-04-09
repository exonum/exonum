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
use failure::ensure;
use toml::Value;

use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{copy, Read, Write},
    panic,
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct ConfigSpec {
    expected_template_file: PathBuf,
    expected_config_dir: PathBuf,
    output_dir: tempfile::TempDir,
    validators_count: usize,
}

impl ConfigSpec {
    const CONFIG_TESTDATA_FOLDER: &'static str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testdata/config");

    fn new(root_dir: impl AsRef<Path>, validators_count: usize) -> Self {
        let root_dir = root_dir.as_ref();
        let expected_template_file = root_dir.join("template.toml");
        let expected_config_dir = root_dir.join("cfg");
        Self {
            expected_template_file,
            expected_config_dir,
            output_dir: tempfile::tempdir().unwrap(),
            validators_count: validators_count,
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

    fn command(&self, name: &str) -> ArgsBuilder {
        ArgsBuilder {
            args: vec!["exonum-config-test".into(), name.into()],
        }
    }

    fn output_dir(&self) -> &Path {
        self.output_dir.as_ref()
    }

    fn output_template_file(&self) -> PathBuf {
        self.output_dir.as_ref().join("template.toml")
    }

    fn output_config_dir(&self, index: usize) -> PathBuf {
        self.output_dir.as_ref().join("cfg").join(index.to_string())
    }

    fn output_pub_config(&self, index: usize) -> PathBuf {
        self.output_config_dir(index).join("pub.toml")
    }

    fn output_pub_configs(&self) -> Vec<PathBuf> {
        (0..self.validators_count)
            .into_iter()
            .map(|i| self.output_pub_config(i))
            .collect()
    }

    fn output_sec_config(&self, index: usize) -> PathBuf {
        self.output_config_dir(index).join("sec.toml")
    }

    fn output_node_config(&self, index: usize) -> PathBuf {
        self.output_config_dir(index).join("node.toml")
    }

    fn expected_config_dir(&self, index: usize) -> PathBuf {
        self.expected_config_dir.join(index.to_string())
    }

    fn expected_node_config(&self, index: usize) -> PathBuf {
        self.expected_config_dir(index).join("node.toml")
    }

    fn copy_config_to_output(&self) {
        eprintln!(
            "Copying from {:?} to {:?}",
            self.expected_config_dir, self.output_dir
        );

        fs_extra::dir::copy(
            &self.expected_config_dir,
            &self.output_dir,
            &fs_extra::dir::CopyOptions::new(),
        )
        .expect("can't copy config");
        fs::copy(&self.expected_template_file, &self.output_template_file())
            .expect("can't copy template");
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
        eprintln!(
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
fn test_generate_template_2() {
    let env = ConfigSpec::new_without_pass();
    let output_template_file = env.output_template_file();
    env.command("generate-template")
        .with_arg(&output_template_file)
        .with_named_arg("--validators-count", env.validators_count.to_string())
        .run()
        .unwrap();
    assert_config_files_eq(&output_template_file, env.expected_template_file);
}

#[test]
fn test_generate_config_relative_pass() {
    unimplemented!()
}

#[test]
fn test_generate_config_ipv4_2() {
    let env = ConfigSpec::new_without_pass();
    env.command("generate-config")
        .with_arg(&env.output_template_file())
        .with_arg(&env.output_config_dir(0))
        .with_named_arg("-a", "127.0.0.1")
        .with_arg("--no-password")
        .run()
        .unwrap()
}

#[test]
fn test_generate_config_ipv6_2() {
    let env = ConfigSpec::new_without_pass();
    env.command("generate-config")
        .with_arg(&env.expected_template_file)
        .with_arg(&env.output_config_dir(0))
        .with_named_arg("-a", "::1")
        .with_arg("--no-password")
        .run()
        .unwrap()
}

#[test]
fn test_generate_full_config_run_without_pass() {
    let env = ConfigSpec::new_without_pass();
    eprintln!("{:#?}", env);
    env.copy_config_to_output();

    env::set_var("EXONUM_CONSENSUS_PASS", "");
    env::set_var("EXONUM_SERVICE_PASS", "");
    for i in 0..env.validators_count {
        let node_config = env.output_node_config(i);
        fs::remove_file(&node_config).unwrap();
        env.command("finalize")
            .with_arg(env.output_sec_config(i))
            .with_arg(&node_config)
            .with_arg("--public-configs")
            .with_args(env.output_pub_configs())
            .run()
            .unwrap();
        assert_config_files_eq(&node_config, env.expected_node_config(i));

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
fn test_generate_full_config_run_with_pass() {
    let env = ConfigSpec::new_with_pass();
    env.copy_config_to_output();

    env::set_var("EXONUM_CONSENSUS_PASS", "some passphrase");
    env::set_var("EXONUM_SERVICE_PASS", "another passphrase");
    let node_config = env.output_node_config(0);
    fs::remove_file(&node_config).unwrap();
    env.command("finalize")
        .with_arg(env.output_sec_config(0))
        .with_arg(&node_config)
        .with_arg("--public-configs")
        .with_args(env.output_pub_configs())
        .run()
        .unwrap();
    assert_config_files_eq(&node_config, env.expected_node_config(0));

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
fn test_different_validators_count() {
    unimplemented!()
}

#[test]
fn test_run_dev() {
    unimplemented!();
}

#[test]
fn test_update_config() {
    let env = ConfigSpec::new_without_pass();
    env.copy_config_to_output();

    let config_path = env.output_node_config(0);

    // Test config update.
    let peer = ConnectInfo {
        address: "0.0.0.1:8080".to_owned(),
        public_key: PublicKey::new([1; PUBLIC_KEY_LENGTH]),
    };

    let connect_list = ConnectListConfig { peers: vec![peer] };

    ConfigManager::update_connect_list(connect_list.clone(), &config_path)
        .expect("Unable to update connect list");
    let config: NodeConfig<PathBuf> =
        ConfigFile::load(&config_path).expect("Can't load node config file");

    let new_connect_list = config.connect_list;
    assert_eq!(new_connect_list.peers, connect_list.peers);
}
