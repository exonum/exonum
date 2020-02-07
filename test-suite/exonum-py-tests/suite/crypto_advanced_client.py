"""Module provides an interface to simplify interaction with cryptocurrency-advanced service."""
import random

from exonum_client import ModuleManager, ExonumClient, MessageGenerator


class ExonumCryptoAdvancedClient:
    """Class provides an interface to simplify interaction with cryptocurrency-advanced service."""

    def __init__(self, client: ExonumClient):
        self.client = client
        cryptocurrency_service_name = "exonum-cryptocurrency-advanced"
        cryptocurrency_service_version = "1.0.0-rc.1"
        self.loader = client.protobuf_loader()
        self.loader.initialize()
        self.loader.load_main_proto_files()
        self.loader.load_service_proto_files(
            runtime_id=0,
            artifact_name="exonum-supervisor",
            artifact_version="1.0.0-rc.1",
        )
        self.loader.load_service_proto_files(
            runtime_id=0,
            artifact_name=cryptocurrency_service_name,
            artifact_version=cryptocurrency_service_version,
        )

        self.cryptocurrency_module = ModuleManager.import_service_module(
            cryptocurrency_service_name, cryptocurrency_service_version, "service"
        )
        self.types_module = ModuleManager.import_service_module(
            cryptocurrency_service_name, cryptocurrency_service_version, "exonum.crypto.types"
        )
        instance_id = client.public_api.get_instance_id_by_name("crypto")
        self.msg_generator = MessageGenerator(
            instance_id=instance_id,
            artifact_name=cryptocurrency_service_name,
            artifact_version=cryptocurrency_service_version,
        )

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.loader.deinitialize()

    def create_wallet(self, keys, wallet_name):
        """Wrapper for create wallet operation."""
        create_wallet = self.cryptocurrency_module.CreateWallet()
        create_wallet.name = wallet_name
        create_wallet_tx = self.msg_generator.create_message(create_wallet)
        create_wallet_tx.sign(keys)
        return self.client.public_api.send_transaction(create_wallet_tx)

    def issue(self, keys, amount):
        """Wrapper for issue operation."""
        issue = self.cryptocurrency_module.Issue()
        issue.amount = amount
        issue.seed = gen_seed()
        issue_tx = self.msg_generator.create_message(issue)
        issue_tx.sign(keys)
        return self.client.public_api.send_transaction(issue_tx)

    def get_wallet_info(self, keys):
        """Wrapper for get wallet info operation."""
        public_service_api = self.client.service_public_api("crypto")
        return public_service_api.get_service(
            "v1/wallets/info?pub_key=" + keys.public_key.hex()
        )

    def get_balance(self, keys):
        wallet = self.get_wallet_info(keys).json()
        return wallet["wallet_proof"]["to_wallet"]["entries"][0]["value"]["balance"]

    def transfer(self, amount, from_wallet, to_wallet):
        """Wrapper for transfer operation."""
        transfer = self.cryptocurrency_module.Transfer()
        transfer.amount = amount
        transfer.seed = gen_seed()
        hash_address = self.msg_generator.pk_to_hash_address(to_wallet)
        transfer.to.CopyFrom(self.types_module.Hash(data=hash_address.value))
        transfer_tx = self.msg_generator.create_message(transfer)
        transfer_tx.sign(from_wallet)
        return self.client.public_api.send_transaction(transfer_tx)


def gen_seed():
    """Method to generate seed"""
    return random.getrandbits(64)
