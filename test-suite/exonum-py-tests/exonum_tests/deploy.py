"""Tests for Exonum service deploy mechanism based on `exonum-launcher` tool."""

import unittest
import time

from exonum_launcher.configuration import Configuration
from exonum_launcher.launcher import Launcher

from suite import run_dev_node, assert_processes_exited_successfully, launcher_networks


class DeployTest(unittest.TestCase):
    """Tests for Exonum deploy process."""

    def test_deploy_run_dev(self):
        """Tests the deploy mechanism in run-dev mode."""
        with run_dev_node("exonum-cryptocurrency-advanced") as network:
            # Wait some time for node to start
            time.sleep(5)

            cryptocurrency_advanced_config_dict = {
                "networks": launcher_networks(network),
                "deadline_height": 10000,
                "artifacts": {"cryptocurrency": {"runtime": "rust", "name": "exonum-cryptocurrency-advanced:0.12.0"}},
                # We aren't testing initialization here.
                "instances": {},
            }

            cryptocurrency_advanced_config = Configuration(cryptocurrency_advanced_config_dict)
            with Launcher(cryptocurrency_advanced_config) as launcher:
                explorer = launcher.explorer()

                launcher.deploy_all()
                launcher.wait_for_deploy()

                for artifact in launcher.launch_state.completed_deployments():
                    deployed = explorer.check_deployed(artifact)
                    self.assertEqual(deployed, True)

            outputs = network.stop()
            assert_processes_exited_successfully(self, outputs)
