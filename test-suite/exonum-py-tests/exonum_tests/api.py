"""Tests for Exonum API."""

import unittest
import time

from exonum_client import ExonumClient

from suite import run_4_nodes, assert_processes_exited_successfully


class ApiTest(unittest.TestCase):
    """Tests for Exonum API."""

    def test_health_info(self):
        """Tests the `healthcheck` endpoint."""
        with run_4_nodes("exonum-cryptocurrency-advanced") as network:
            # Wait some time for nodes to start
            time.sleep(5)

            for validator_id in range(network.validators_count()):
                host, public_port, private_port = network.api_address(validator_id)
                client = ExonumClient(host, public_port, private_port)

                health_info_response = client.health_info()

                self.assertEqual(health_info_response.status_code, 200)

            outputs = network.stop()
            assert_processes_exited_successfully(self, outputs)
