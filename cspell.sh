#!/usr/bin/env bash

# Script running cspell checks on all the specified project directories.

# Copyright 2018 The Exonum Team
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

set -e

./node_modules/.bin/cspell sandbox/{src,examples,tests}/**/*.rs
./node_modules/.bin/cspell exonum/{src,benches,tests}/**/*.rs
./node_modules/.bin/cspell exonum/{src,benches,tests}/**/**/*.rs
./node_modules/.bin/cspell exonum/fuzz/fuzz_targets/*.rs
./node_modules/.bin/cspell testkit/{src,examples,tests}/**/*.rs
./node_modules/.bin/cspell testkit/server/{src,examples,tests}/**/*.rs
./node_modules/.bin/cspell services/configuration/{src,examples}/**/*.rs
./node_modules/.bin/cspell services/time/{src,examples,tests}/**/*.rs
./node_modules/.bin/cspell examples/cryptocurrency/{src,examples,tests}/**/*.rs
./node_modules/.bin/cspell examples/cryptocurrency-advanced/{src,examples,tests}/**/*.rs
./node_modules/.bin/cspell examples/timestamping/{src,examples,tests}/**/*.rs
find . -not -path "./3rdparty/*" -and -not -path "./node_modules/*" -name "*.md" | xargs ./node_modules/.bin/cspell
