# Exonum Python Integration Tests

This module contains a library providing interface for building
integration tests for `Exonum`, and a set of integration tests.

Library for writing tests can be found in the [`suite`](suite) directory,
and tests can be found in [`exonum_tests`](exonum_tests) directory.

## Description

`exonum-py-tests` consists of two parts:

- `suite`: A library providing interface for bootstrapping and launching
  Exonum network, which rely on [`exonum-launcher`] and
  [`exonum-python-client`] projects.
- `exonum_tests`: Set of integration tests for Exonum, built atop of the
  `unittest` library.

## Examples

Example of basic test that uses `suite`:

```python
import unittest

from exonum_client import ExonumClient

from suite import (
    run_4_nodes,
    assert_processes_exited_successfully,
    wait_network_to_start,
)

class ApiTest(unittest.TestCase):
    def test_block_response(self):
        """Tests the `block` endpoint. Check response for block"""

        # Bootstrap the network of 4 nodes with
        # `exonum-cryptocurrency-advanced` service
        with run_4_nodes("exonum-cryptocurrency-advanced") as network:
            # Since we're actually running the nodes,
            # we have to wait until nodes start.
            wait_network_to_start(self.network)
            # We can iterate through validators in the network.
            for validator_id in range(network.validators_count()):
                # For every validator, all the connection
                # information is available.
                host, public_port, private_port = network.api_address(validator_id)
                # For interaction with nodes we can use `ExonumClient`.
                client = ExonumClient(host, public_port, private_port)
                block_response = client.public_api.get_block(1)

                # Testing is performed as usual in `unittest`.
                self.assertEqual(block_response.status_code, 200)
                self.assertEqual(block_response.json()['height'], 1)
                self.assertEqual(block_response.json()['tx_count'], 0)
                self.assertIsNotNone(block_response.json()['time'])

            # After usage, we stop all the nodes and check if
            # they exited successfully.
            outputs = network.stop()
            assert_processes_exited_successfully(self, outputs)
```

## Usage

Install the package (`test-suite/exonum-py-tests` here stands for path
to the `exonum-py-tests` directory, not the package name):

```sh
# It is recommended to work in `venv`
python3 -m venv .venv
source .venv/bin/activate
# Clone the `exonum-launcher` to get the latest version
# compatible with `master` branch of Exonum.
git clone https://github.com/exonum/exonum-launcher.git .venv/exonum-launcher
# Install pip (if required).
pip install pip --upgrade
# Install dependencies from github-provided exonum-launcher
# (so we can get latest changes without release).
pip install -r .venv/exonum-launcher/requirements.txt
# Install exonum-launcher itself from the cloned repository as well.
pip install -e .venv/exonum-launcher
# Install integration tests.
pip install -e test-suite/exonum-py-tests --no-binary=protobuf protobuf
```

Also ensure that you have freshly installed `cryptocurrency-advanced` example.

Run tests:

```sh
python3 -m exonum_tests
```

## LICENSE

`exonum-py-tests` is licensed under the Apache License (Version 2.0).
See [LICENSE] for details.

[LICENSE]: https://github.com/exonum/exonum/blob/master/LICENSE
[`exonum-launcher`]: https://github.com/exonum/exonum-launcher
[`exonum-python-client`]: https://github.com/exonum/exonum-python-client
