import unittest

from exonum_client import ExonumClient
from exonum_client.crypto import KeyPair
from exonum_launcher.configuration import Configuration
from exonum_launcher.launcher import Launcher

from suite import (
    assert_processes_exited_successfully,
    run_4_nodes,
    wait_network_to_start,
    ExonumCryptoAdvancedClient,
    generate_config,
)


class FreezeTests(unittest.TestCase):
    """Tests for a checking service freezing mechanism."""

    def setUp(self):
        self.network = run_4_nodes("exonum-cryptocurrency-advanced")
        self.addCleanup(self._tear_down, False)
        wait_network_to_start(self.network)

    def test_freeze_service(self):
        host, public_port, private_port = self.network.api_address(0)
        client = ExonumClient(host, public_port, private_port)

        # Create wallet
        alice_keys = KeyPair.generate()
        with ExonumCryptoAdvancedClient(client) as crypto_client:
            crypto_client.create_wallet(alice_keys, "Alice")
            with client.create_subscriber("transactions") as subscriber:
                subscriber.wait_for_new_event()
                alice_balance = crypto_client.get_balance(alice_keys)
                self.assertEqual(alice_balance, 100)

        # Freeze the service
        instances = {"crypto": {"artifact": "cryptocurrency", "action": "freeze"}}
        cryptocurrency_advanced_config_dict = generate_config(self.network, instances=instances, artifact_action="none")

        cryptocurrency_advanced_config = Configuration(cryptocurrency_advanced_config_dict)
        with Launcher(cryptocurrency_advanced_config) as launcher:
            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

        # Check that the service status has been changed to `frozen`.
        for service in client.public_api.available_services().json()["services"]:
            if service["spec"]["name"] == "crypto":
                self.assertEqual(service["status"]["type"], "frozen")

        # Try to create a new wallet. The operation should fail.
        with ExonumCryptoAdvancedClient(client) as crypto_client:
            bob_keys = KeyPair.generate()
            response = crypto_client.create_wallet(bob_keys, "Bob")
            self.assertEqual(response.status_code, 400)
            # Because the service is frozen, transaction should be inadmissible.
            self.assertEqual(response.json()["title"], "Failed to add transaction to memory pool")

        # Check that we can use service endpoints for data retrieving. Check wallet once again.
        with ExonumCryptoAdvancedClient(client) as crypto_client:
            alice_balance = crypto_client.get_balance(alice_keys)
            self.assertEqual(alice_balance, 100)

    def test_resume_after_freeze_service(self):
        host, public_port, private_port = self.network.api_address(0)
        client = ExonumClient(host, public_port, private_port)

        # Create wallet
        with ExonumCryptoAdvancedClient(client) as crypto_client:
            alice_keys = KeyPair.generate()
            crypto_client.create_wallet(alice_keys, "Alice")
            with client.create_subscriber("transactions") as subscriber:
                subscriber.wait_for_new_event()
                alice_balance = crypto_client.get_balance(alice_keys)
                self.assertEqual(alice_balance, 100)

        # Freeze the service
        instances = {"crypto": {"artifact": "cryptocurrency", "action": "freeze"}}
        cryptocurrency_advanced_config_dict = generate_config(self.network, instances=instances, artifact_action="none")
        cryptocurrency_advanced_config = Configuration(cryptocurrency_advanced_config_dict)
        with Launcher(cryptocurrency_advanced_config) as launcher:
            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

        # Resume the service
        instances = {"crypto": {"artifact": "cryptocurrency", "action": "resume"}}
        cryptocurrency_advanced_config_dict = generate_config(self.network, instances=instances, artifact_action="none")
        cryptocurrency_advanced_config = Configuration(cryptocurrency_advanced_config_dict)
        with Launcher(cryptocurrency_advanced_config) as launcher:
            launcher.deploy_all()
            launcher.wait_for_deploy()
            launcher.start_all()
            launcher.wait_for_start()

        # Check that the service status has been changed to `active`.
        for service in client.public_api.available_services().json()["services"]:
            if service["spec"]["name"] == "crypto":
                self.assertEqual(service["status"]["type"], "active")

        # Check that an ability to create wallets has been restored.
        with ExonumCryptoAdvancedClient(client) as crypto_client:
            bob_keys = KeyPair.generate()
            crypto_client.create_wallet(bob_keys, "Bob")
            with client.create_subscriber("transactions") as subscriber:
                subscriber.wait_for_new_event()
                bob_balance = crypto_client.get_balance(bob_keys)
                self.assertEqual(bob_balance, 100)

    def _tear_down(self, check_exit_codes=True):
        """Performs cleanup, removing network files."""

        if self.network is not None:
            outputs = self.network.stop()
            self.network.deinitialize()
            self.network = None

            if check_exit_codes:
                assert_processes_exited_successfully(self, outputs)

    def tearDown(self):
        self._tear_down()
