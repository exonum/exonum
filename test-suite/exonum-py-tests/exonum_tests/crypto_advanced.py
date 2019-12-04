"""Tests for cryptocurrency-advanced service"""
import unittest
import time

from exonum_client import ExonumClient
from exonum_client.crypto import KeyPair
from exonum_launcher.configuration import Configuration
from exonum_launcher.launcher import Launcher

from suite import assert_processes_exited_successfully, \
  launcher_networks, run_4_nodes, ExonumCryptoAdvancedClient


class CryptoAdvancedTest(unittest.TestCase):
    """Tests for Cryptocurrency Advanced"""

    def setUp(self):
        self.network = run_4_nodes("exonum-cryptocurrency-advanced")
        time.sleep(3)
        cryptocurrency_advanced_config_dict = {
          "networks": launcher_networks(self.network),
          "deadline_height": 10000,
          "artifacts": {"cryptocurrency": {"runtime": "rust", "name": "exonum-cryptocurrency-advanced:0.13.0-rc.1"}},
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

            for instance in launcher.launch_state.completed_initializations():
                explorer.wait_for_start(instance)

    def test_create_wallet(self):
        """Tests the wallet creation"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                with client.create_subscriber() as subscriber:
                    subscriber.wait_for_new_block()
                self.assertEqual(crypto_client.get_wallet_info(alice_keys).status_code, 200)
                # TODO: Sometimes it fails without time.sleep() [ECR-3876]
                time.sleep(2)
                alice_balance = (crypto_client.get_wallet_info(alice_keys).json()
                ['wallet_proof']['to_wallet']['entries'][0]['value']['balance'])
                self.assertEqual(alice_balance, 100)

    def test_token_issue(self):
        """Tests the token issue"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                with client.create_subscriber() as subscriber:
                    subscriber.wait_for_new_block()
                    # TODO: Sometimes it fails without time.sleep() [ECR-3876]
                    time.sleep(2)
                    crypto_client.issue(alice_keys, 100)
                    subscriber.wait_for_new_block()
                    # TODO: Sometimes it fails without time.sleep() [ECR-3876]
                    time.sleep(2)
                    alice_balance = (crypto_client.get_wallet_info(alice_keys).json()
                                     ['wallet_proof']['to_wallet']['entries'][0]['value']['balance'])
                    self.assertEqual(alice_balance, 200)

    def test_transfer_funds(self):
        """Tests the transfer funds to another wallet"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                bob_keys = KeyPair.generate()
                crypto_client.create_wallet(bob_keys, "Bob" + str(validator_id))
                with client.create_subscriber() as subscriber:
                    subscriber.wait_for_new_block()
                    # TODO: Sometimes it fails without time.sleep() [ECR-3876]
                    time.sleep(2)
                    crypto_client.transfer(20, alice_keys, bob_keys.public_key.value)
                    subscriber.wait_for_new_block()
                    # TODO: Sometimes it fails without time.sleep() [ECR-3876]
                    time.sleep(2)
                    alice_balance = (crypto_client.get_wallet_info(alice_keys).json()
                                     ['wallet_proof']['to_wallet']['entries'][0]['value']['balance'])
                    bob_balance = (crypto_client.get_wallet_info(bob_keys).json()
                                   ['wallet_proof']['to_wallet']['entries'][0]['value']['balance'])
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
                with client.create_subscriber() as subscriber:
                    subscriber.wait_for_new_block()
                    crypto_client.transfer(10, alice_keys, alice_keys.public_key.value)
                    subscriber.wait_for_new_block()
                    alice_balance = (crypto_client.get_wallet_info(alice_keys).json()
                                     ['wallet_proof']['to_wallet']['entries'][0]['value']['balance'])
                    self.assertEqual(alice_balance, 100)

    def test_create_wallet_same_name(self):
        """Tests the transaction with the same wallet name is rejected"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                with client.create_subscriber() as subscriber:
                    subscriber.wait_for_new_block()
                # create the wallet with the same name again
                crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                with client.create_subscriber() as subscriber:
                    subscriber.wait_for_new_block()
        # it should contain 4 txs for wallet creation plus 6 services txs
        self.assertEqual(client.stats().json()['tx_count'], 10)

    def test_create_wallet_unique_for_key_pair(self):
        """Tests the transaction with the same keys for different wallets is failed"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                tx_response = crypto_client.create_wallet(alice_keys, "Alice" + str(validator_id))
                with client.create_subscriber() as subscriber:
                    subscriber.wait_for_new_block()
                    # TODO: Sometimes it fails without time.sleep() [ECR-3876]
                    time.sleep(2)
                tx_status = client.get_tx_info(tx_response.json()['tx_hash']).json()['status']['type']
                self.assertEqual(tx_status, 'success')
                # create the wallet with the same keys again
                tx_same_keys = crypto_client.create_wallet(alice_keys, "Alice_Dublicate" + str(validator_id))
                with client.create_subscriber() as subscriber:
                    subscriber.wait_for_new_block()
                tx_status = client.get_tx_info(tx_same_keys.json()['tx_hash']).json()['status']['type']
                self.assertEqual(tx_status, 'service_error')

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
                with client.create_subscriber() as subscriber:
                    subscriber.wait_for_new_block()
                    tx_response = crypto_client.transfer(110, alice_keys, bob_keys.public_key.value)
                    subscriber.wait_for_new_block()
                    tx_status = client.get_tx_info(tx_response.json()['tx_hash']).json()['status']['type']
                    self.assertEqual(tx_status, 'service_error')
                    alice_balance = (crypto_client.get_wallet_info(alice_keys).json()
                                     ['wallet_proof']['to_wallet']['entries'][0]['value']['balance'])
                    bob_balance = (crypto_client.get_wallet_info(bob_keys).json()
                                   ['wallet_proof']['to_wallet']['entries'][0]['value']['balance'])
                    self.assertEqual(alice_balance, 100)
                    self.assertEqual(bob_balance, 100)

    def test_get_nonexistent_wallet(self):
        """Tests the wallet history is None for nonexistent wallet"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                wallet_history = crypto_client.get_wallet_info(alice_keys).json()['wallet_history']
                self.assertIsNone(wallet_history)

    def test_add_funds_to_nonexistent_wallet(self):
        """Tests the funds issue is failed if wallet doesn't exist"""

        for validator_id in range(self.network.validators_count()):
            host, public_port, private_port = self.network.api_address(validator_id)
            client = ExonumClient(host, public_port, private_port)
            with ExonumCryptoAdvancedClient(client) as crypto_client:
                alice_keys = KeyPair.generate()
                tx_response = crypto_client.issue(alice_keys, 100)
                with client.create_subscriber() as subscriber:
                    subscriber.wait_for_new_block()
                    # TODO: Sometimes it fails without time.sleep() [ECR-3876]
                    time.sleep(2)
                    tx_status = client.get_tx_info(tx_response.json()['tx_hash']).json()['status']['type']
                    self.assertEqual(tx_status, 'service_error')

    def tearDown(self):
        outputs = self.network.stop()
        assert_processes_exited_successfully(self, outputs)
