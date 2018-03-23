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

// This file contains compiliation tests to cover transaction! macro.
// See the following:
// https://github.com/laumann/compiletest-rs

extern crate compiletest_rs as compiletest;

use std::path::PathBuf;

fn run_mode(mode: &'static str) {
    let mut config = compiletest::Config::default();

    config.mode = mode.parse().expect("Invalid mode");
    config.src_base = PathBuf::from(format!("tests/{}", mode));
    // Populate config.target_rustcflags with dependencies on the path
    config.link_deps();
    // If your tests import the parent crate, this helps with E0464
    config.clean_rmeta();

    compiletest::run_tests(&config);
}

#[test]
fn compile_test() {
    run_mode("compile-fail");
    run_mode("run-pass");
}
