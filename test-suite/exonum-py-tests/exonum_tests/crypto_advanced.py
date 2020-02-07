"""Tests for cryptocurrency-advanced service"""
import unittest

from exonum_client import ExonumClient
from exonum_client.crypto import KeyPair
from exonum_launcher.configuration import Configuration
from exonum_launcher.launcher import Launcher

from suite import (
    assert_processes_exited_successfully,
    launcher_networks,
    run_4_nodes,
    wait_network_to_start,
    ExonumCryptoAdvancedClient,
    generate_config,
)


class CryptoAdvancedTest(unittest.TestCase):
    """Tests for Cryptocurrency Advanced"""

    def setUp(self):
        try:
            self.network = run_4_nodes("exonum-cryptocurrency-advanced")
            wait_network_to_start(self.network)

            instances = {"crypto": {"artifact": "cryptocurrency"}}
            cryptocurrency_advanced_config_dict = generate_config(
                self.network, instances=instances
            )

            cryptocurrency_advanced_config = Configuration(
                cryptocurrency_advanced_config_dict
            )
            with Launcher(cryptocurrency_advanced_config) as launcher:
                explorer = launcher.explorer()

                # Skip deploy and start. The service has been already included.
                # launcher.deploy_all()
                # launcher.wait_for_deploy()
                # launcher.start_all()
                # launcher.wait_for_start()

                for artifact in launcher.launch_state.completed_deployments():
                    deployed = explorer.check_deployed(artifact)
                    self.assertEqual(deployed, True)

                # Launcher checks that config is applied, no need to check it again.
        except Exception as error:
            # If exception is raise in `setUp`, `tearDown` won't be called,
            # thus here we ensure that network is stopped and temporary data is removed.
            # Then we re-raise exception, since the test should fail.
            self.network.stop()
            self.network.deinitialize()
            raise error

    def test_create_wallet(self):
        """Tests the wallet creation"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                with client.create_subscriber("transactions") as subscriber:
                    subscriber.wait_for_new_event()
                self.assertEqual(
                    crypto_client.get_wallet_info(alice_keys).status_code, 200
                )
                alice_balance = crypto_client.get_balance(alice_keys)
                self.assertEqual(alice_balance, 100)

    def test_token_issue(self):
        """Tests the token issue"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                with client.create_subscriber("transactions") as subscriber:
                    subscriber.wait_for_new_event()
                    crypto_client.issue(alice_keys, 100)
                    subscriber.wait_for_new_event()
                    alice_balance = crypto_client.get_balance(alice_keys)
                    self.assertEqual(alice_balance, 200)

    def test_transfer_funds(self):
        """Tests the transfer funds to another wallet"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                bob_keys = KeyPair.generate()
                with client.create_subscriber("transactions") as subscriber:
                    crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                    subscriber.wait_for_new_event()
                    crypto_client.create_wallet(bob_keys, "Bob" + str(validator_id))
                    subscriber.wait_for_new_event()
                    crypto_client.transfer(20, alice_keys, bob_keys.public_key)
                    subscriber.wait_for_new_event()
                    alice_balance = crypto_client.get_balance(alice_keys)
                    bob_balance = crypto_client.get_balance(bob_keys)
                    self.assertEqual(alice_balance, 80)
                    self.assertEqual(bob_balance, 120)

    def test_transfer_to_yourself(self):
        """Tests the transfer funds to yourself is impossible"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                with client.create_subscriber("transactions") as subscriber:
                    subscriber.wait_for_new_event()
                    crypto_client.transfer(10, alice_keys, alice_keys.public_key)
                    subscriber.wait_for_new_event()
                    alice_balance = crypto_client.get_balance(alice_keys)
                    self.assertEqual(alice_balance, 100)

    def test_create_wallet_same_name(self):
        """Tests the transaction with the same wallet name is rejected"""
        client = None
        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                with client.create_subscriber("transactions") as subscriber:
                    subscriber.wait_for_new_event()
                # create the wallet with the same name again
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                with client.create_subscriber("blocks") as subscriber:
                    subscriber.wait_for_new_event()
        # it should contain 4 txs for wallet creation
        self.assertEqual(client.public_api.stats().json()["tx_count"], 4)

    def test_create_wallet_unique_for_key_pair(self):
        """Tests the transaction with the same keys for different wallets is failed"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                tx_response = crypto_client.create_wallet(
                    alice_keys, "Alice" + str(validator_id)
                )
                with client.create_subscriber("transactions") as subscriber:
                    subscriber.wait_for_new_event()
                tx_status = client.public_api.get_tx_info(
                    tx_response.json()["tx_hash"]
                ).json()["status"]["type"]
                self.assertEqual(tx_status, "success")
                # create the wallet with the same keys again
                tx_same_keys = crypto_client.create_wallet(
                    alice_keys, "Alice_Dublicate" + str(validator_id)
                )
                with client.create_subscriber("blocks") as subscriber:
                    subscriber.wait_for_new_event()
                tx_status = client.public_api.get_tx_info(
                    tx_same_keys.json()["tx_hash"]
                ).json()["status"]["type"]
                self.assertEqual(tx_status, "service_error")

    def test_transfer_funds_insufficient(self):
        """Tests the transfer insufficient amount of funds is failed"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                bob_keys = KeyPair.generate()
                crypto_client.create_wallet(bob_keys, "Bob" + str(validator_id))
                with client.create_subscriber("blocks") as subscriber:
                    subscriber.wait_for_new_event()
                    tx_response = crypto_client.transfer(
                        110, alice_keys, bob_keys.public_key
                    )
                    subscriber.wait_for_new_event()
                    tx_info = client.public_api.get_tx_info(
                        tx_response.json()["tx_hash"]
                    ).json()
                    tx_status = tx_info["status"]["type"]
                    self.assertEqual(tx_status, "service_error")
                    alice_balance = crypto_client.get_balance(alice_keys)
                    bob_balance = crypto_client.get_balance(bob_keys)
                    self.assertEqual(alice_balance, 100)
                    self.assertEqual(bob_balance, 100)

    def test_get_nonexistent_wallet(self):
        """Tests the wallet history is None for nonexistent wallet"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                wallet_history = crypto_client.get_wallet_info(alice_keys).json()[
                    "wallet_history"
                ]
                self.assertIsNone(wallet_history)

    def test_add_funds_to_nonexistent_wallet(self):
        """Tests the funds issue is failed if wallet doesn't exist"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                tx_response = crypto_client.issue(alice_keys, 100)
                with client.create_subscriber("transactions") as subscriber:
                    subscriber.wait_for_new_event()
                    tx_info = client.public_api.get_tx_info(
                        tx_response.json()["tx_hash"]
                    ).json()
                    tx_status = tx_info["status"]["type"]
                    self.assertEqual(tx_status, "service_error")

    def tearDown(self):
        outputs = self.network.stop()
        assert_processes_exited_successfully(self, outputs)
        self.network.deinitialize()
