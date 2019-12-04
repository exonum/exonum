# Exonum Python Integration Tests

This module contains a library providing interface for building
integration tests for `Exonum`, and a set of integration tests.

Library for writing tests can be found in the [`suite`](suite) directory,
and tests can be found in [`exonum_tests`](exonum_tests) directory.

## Usage

Install the package (`test-suite/exonum-py-tests` here stands for path
to the `exonum-py-tests` directory, not the package name):

```sh
python3 -m venv .venv
source .venv/bin/activate
git clone https://github.com/exonum/exonum-launcher.git .venv/exonum-launcher
pip install pip --upgrade
# Install dependencies from github-provided exonum-launcher (so we can get latest changes without release).
pip install -r .venv/exonum-launcher/requirements.txt
# Install exonum-launcher itself from the cloned repo as well.
pip install -e .venv/exonum-launcher
# Install integration tests.
pip install -e test-suite/exonum-py-tests --no-binary=protobuf protobuf
```

Also ensure that you have freshly installed `cryptocurrency-advanced` example.

Run tests:

```sh
python3 -m exonum_tests
```
