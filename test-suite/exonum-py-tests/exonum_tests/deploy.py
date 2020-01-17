"""Tests for Exonum service deploy mechanism based on `exonum-launcher` tool."""

import unittest

import re
from exonum_client import ExonumClient
from exonum_client.crypto import KeyPair
from exonum_launcher.configuration import Configuration
from exonum_launcher.launcher import Launcher
from exonum_launcher.explorer import ExecutionFailError

from suite import (
    run_dev_node,
    assert_processes_exited_successfully,
    launcher_networks,
    run_4_nodes,
    wait_network_to_start,
    ExonumCryptoAdvancedClient
)


class RegularDeployTest(unittest.TestCase):
    """Tests for Exonum deploy process in regular mode."""

    def setUp(self):
        self.network = run_4_nodes("exonum-cryptocurrency-advanced")
        wait_network_to_start(self.network)

    def test_deploy_regular(self):
        """Tests the deploy mechanism in regular mode."""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            # We aren't testing initialization here.
            "instances": {},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()

            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, True)

    def test_deploy_regular_invalid_artifact_name(self):
        """Tests the deploy mechanism in regular mode with invalid artifact"""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "test-service",
                    "version": "0.13.0-rc.2",
                }
            },
            # We aren't testing initialization here.
            "instances": {},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()

            # invalid artifact should not be deployed
            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, False)

    def test_deploy_regular_exceed_deadline_height(self):
        """Tests the deploy mechanism in regular mode with exceeded deadline height"""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 0,
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            # We aren't testing initialization here.
            "instances": {},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            with self.assertRaises(ExecutionFailError):
                launcher.wait_for_deploy()

            # artifact should not be deployed because of exceeded deadline height
            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, False)

    def test_deploy_regular_with_instance(self):
        """Tests the deploy mechanism in regular mode with instance."""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            "instances": {"crypto": {"artifact": "cryptocurrency"}},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, True)

            self.assertEqual(len(launcher.launch_state.completed_configs()), 1)

    def test_deploy_regular_with_consensus_config(self):
        """Tests the deploy mechanism in regular mode with consensus config."""

        pub_configs = self.network._public_configs().split()
        validator_keys = []
        for pub_config in pub_configs:
            keys = []
            with open(pub_config, 'r') as file:
                data = file.read()
                keys.append(re.search('consensus_key = "(.+?)"', data).group(1))
                keys.append(re.search('service_key = "(.+?)"', data).group(1))
            validator_keys.append(keys)

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "consensus": {
              "validator_keys": validator_keys,
              "first_round_timeout": 3000,
              "status_timeout": 5000,
              "peers_timeout": 10000,
              "txs_block_limit": 5000,
              "max_message_len": 1048576,
              "min_propose_timeout": 10,
              "max_propose_timeout": 200,
              "propose_timeout_threshold": 500
            },
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            "instances": {"crypto": {"artifact": "cryptocurrency"}},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, True)

            self.assertEqual(len(launcher.launch_state.completed_configs()), 1)

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            supervisor_api = client.service_apis("supervisor")
            consensus_config = supervisor_api[0].get_service("consensus-config").json()
            # check that initial config has been applied
            self.assertEqual(consensus_config['txs_block_limit'], 5000)

    def test_deploy_regular_with_invalid_consensus_config(self):
        """Tests the deploy mechanism in regular mode with
        invalid consensus config."""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "consensus": {
              "first_round_timeout": 3000,
              "status_timeout": 5000,
              "peers_timeout": 10000,
              "txs_block_limit": 1000,
              "max_message_len": 1048576,
              "min_propose_timeout": 10,
              "max_propose_timeout": 200,
              "propose_timeout_threshold": 500
            },
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            "instances": {"crypto": {"artifact": "cryptocurrency"}},
        }

        with self.assertRaises(RuntimeError):
            Configuration(
                cryptocurrency_advanced_config_dict)

    def test_deploy_regular_stop_running_instance(self):
        """Tests the deploy mechanism to stop running instance."""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            "instances": {"crypto": {"artifact": "cryptocurrency"}},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, True)

            self.assertEqual(len(launcher.launch_state.completed_configs()), 1)

        # stop service
        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "artifacts": {
              "cryptocurrency": {
                "runtime": "rust",
                "name": "exonum-cryptocurrency-advanced",
                "version": "0.13.0-rc.2",
              }
            },
            "instances": {"crypto": {"artifact": "cryptocurrency", "action": "stop"}},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:

            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            available_services = client.public_api.available_services().json()
            # crypto instance always first element in array
            self.assertEqual(available_services['services'][0]['status']['type'], 'stopped')
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                tx_response = crypto_client.create_wallet(
                    alice_keys, "Alice" + str(validator_id)
                )
                # in case of stopped service its tx will not be processed
                self.assertEqual(tx_response.status_code, 400)
                self.assertIn("Specified service is not active", str(tx_response.content))

    def test_deploy_regular_with_instance_stop_action_before_start(self):
        """Tests the deploy mechanism in regular mode with instance
        within stop action before start."""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            "instances": {"crypto": {"artifact": "cryptocurrency", "action": "stop"}},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:

            launcher.deploy_all()
            launcher.wait_for_deploy()
            with self.assertRaises(RuntimeError):
                launcher.start_all()

    def test_deploy_regular_with_invalid_instance(self):
        """Tests the deploy mechanism in regular mode with invalid instance."""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            "instances": {"": {"artifact": "cryptocurrency"}},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            with self.assertRaises(ExecutionFailError):
                launcher.wait_for_start()

            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, True)

    def test_deploy_regular_with_invalid_action(self):
        """Tests the deploy mechanism in regular mode with
        invalid action."""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            "instances": {"crypto": {"artifact": "cryptocurrency", "action": "invalid_action"}},
        }

        with self.assertRaises(RuntimeError):
            Configuration(
                cryptocurrency_advanced_config_dict)

    def tearDown(self):
        outputs = self.network.stop()
        assert_processes_exited_successfully(self, outputs)


class DevDeployTest(unittest.TestCase):
    """Tests for Exonum deploy process in dev mode."""

    def setUp(self):
        self.network = run_dev_node("exonum-cryptocurrency-advanced")
        wait_network_to_start(self.network)

    def test_deploy_run_dev(self):
        """Tests the deploy mechanism in run-dev mode."""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            # We aren't testing initialization here.
            "instances": {},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()

            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, True)

    def test_deploy_dev_with_instance(self):
        """Tests the deploy mechanism in dev mode with instance."""

        cryptocurrency_advanced_config_dict = {
            "networks": launcher_networks(self.network),
            "deadline_height": 10000,
            "artifacts": {
                "cryptocurrency": {
                    "runtime": "rust",
                    "name": "exonum-cryptocurrency-advanced",
                    "version": "0.13.0-rc.2",
                }
            },
            "instances": {"crypto": {"artifact": "cryptocurrency"}},
        }

        cryptocurrency_advanced_config = Configuration(
            cryptocurrency_advanced_config_dict
        )
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, True)

            self.assertEqual(len(launcher.launch_state.completed_configs()), 1)

    def tearDown(self):
        outputs = self.network.stop()
        assert_processes_exited_successfully(self, outputs)
