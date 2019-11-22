# Exonum Python Integration Tests

This module contains a library providing interface for building
integration tests for `Exonum`, and a set of integration tests.

Library for writing tests can be found in the [`suite`](suite) directory,
and tests can be found in [`exonum_tests`](exonum_tests) directory.

## Usage

Install the package (`exonum-py-tests` here stands for path
to the `exonum-py-tests` directory, not the package name):

```sh
pip install -e exonum-py-tests
```

Run tests:

```sh
python3 -m exonum_tests
```
