"""Tests for Exonum service deploy mechanism based on `exonum-launcher` tool."""

import unittest
import time

from exonum_launcher.configuration import Configuration
from exonum_launcher.launcher import Launcher
from exonum_launcher.action_result import ActionResult

from suite import run_dev_node, assert_processes_exited_successfully, launcher_networks, run_4_nodes


class RegularDeployTest(unittest.TestCase):
    """Tests for Exonum deploy process in regular mode."""

    def setUp(self):
        self.network = run_4_nodes("exonum-cryptocurrency-advanced")
        time.sleep(3)

    def test_deploy_regular(self):
        """Tests the deploy mechanism in regular mode."""

        cryptocurrency_advanced_config_dict = {
          "networks": launcher_networks(self.network),
          "deadline_height": 10000,
          "artifacts": {"cryptocurrency": {"runtime": "rust", "name": "exonum-cryptocurrency-advanced:0.13.0-rc.2"}},
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

    def test_deploy_regular_invalid_artifact_name(self):
        """Tests the deploy mechanism in regular mode with invalid artifact"""

        cryptocurrency_advanced_config_dict = {
          "networks": launcher_networks(self.network),
          "deadline_height": 10000,
          "artifacts": {"cryptocurrency": {"runtime": "rust", "name": "test-service:0.13.0-rc.2"}},
          # We aren't testing initialization here.
          "instances": {},
        }

        cryptocurrency_advanced_config = Configuration(cryptocurrency_advanced_config_dict)
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
          "artifacts": {"cryptocurrency": {"runtime": "rust", "name": "exonum-cryptocurrency-advanced:0.13.0-rc.2"}},
          # We aren't testing initialization here.
          "instances": {},
        }

        cryptocurrency_advanced_config = Configuration(cryptocurrency_advanced_config_dict)
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
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
          "artifacts": {"cryptocurrency": {"runtime": "rust", "name": "exonum-cryptocurrency-advanced:0.13.0-rc.2"}},
          "instances": {"crypto": {"artifact": "cryptocurrency"}},
        }

        cryptocurrency_advanced_config = Configuration(cryptocurrency_advanced_config_dict)
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, True)

            self.assertEqual(len(launcher.launch_state.completed_initializations()), 1)
            for instance in launcher.launch_state.completed_initializations():
                self.assertEqual(explorer.wait_for_start(instance), ActionResult.Success)

    def test_deploy_regular_with_invalid_instance(self):
        """Tests the deploy mechanism in regular mode with invalid instance."""

        cryptocurrency_advanced_config_dict = {
          "networks": launcher_networks(self.network),
          "deadline_height": 10000,
          "artifacts": {"cryptocurrency": {"runtime": "rust", "name": "exonum-cryptocurrency-advanced:0.13.0-rc.2"}},
          "instances": {"": {"artifact": "cryptocurrency"}},
        }

        cryptocurrency_advanced_config = Configuration(cryptocurrency_advanced_config_dict)
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, True)

            self.assertEqual(len(launcher.launch_state.completed_initializations()), 1)
            for instance in launcher.launch_state.completed_initializations():
                self.assertEqual(explorer.wait_for_start(instance), ActionResult.Fail)

    def tearDown(self):
        outputs = self.network.stop()
        assert_processes_exited_successfully(self, outputs)


class DevDeployTest(unittest.TestCase):
    """Tests for Exonum deploy process in dev mode."""

    def setUp(self):
        self.network = run_dev_node("exonum-cryptocurrency-advanced")
        time.sleep(3)

    def test_deploy_run_dev(self):
        """Tests the deploy mechanism in run-dev mode."""

        cryptocurrency_advanced_config_dict = {
          "networks": launcher_networks(self.network),
          "deadline_height": 10000,
          "artifacts": {"cryptocurrency": {"runtime": "rust", "name": "exonum-cryptocurrency-advanced:0.13.0-rc.2"}},
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

    def test_deploy_dev_with_instance(self):
        """Tests the deploy mechanism in dev mode with instance."""

        cryptocurrency_advanced_config_dict = {
          "networks": launcher_networks(self.network),
          "deadline_height": 10000,
          "artifacts": {"cryptocurrency": {"runtime": "rust", "name": "exonum-cryptocurrency-advanced:0.13.0-rc.2"}},
          "instances": {"crypto": {"artifact": "cryptocurrency"}},
        }

        cryptocurrency_advanced_config = Configuration(cryptocurrency_advanced_config_dict)
        with Launcher(cryptocurrency_advanced_config) as launcher:
            explorer = launcher.explorer()

            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

            for artifact in launcher.launch_state.completed_deployments():
                deployed = explorer.check_deployed(artifact)
                self.assertEqual(deployed, True)

            self.assertEqual(len(launcher.launch_state.completed_initializations()), 1)
            for instance in launcher.launch_state.completed_initializations():
                self.assertEqual(explorer.wait_for_start(instance), ActionResult.Success)

    def tearDown(self):
        outputs = self.network.stop()
        assert_processes_exited_successfully(self, outputs)
