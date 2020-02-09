"""Module containing common scenarios that can be used
for writing tests with less boiler-plate."""

from typing import List, Dict, Any
import unittest

import time
from exonum_client import ExonumClient
from suite import ExonumNetwork, ProcessOutput, ProcessExitResult
from requests.exceptions import ConnectionError

RETRIES_AMOUNT = 20
ARTIFACT_NAME = "exonum-cryptocurrency-advanced"
ARTIFACT_VERSION = "1.0.0-rc.1"


def run_dev_node(application: str) -> ExonumNetwork:
    """Starts a single node in the run-dev mode and returns
    `ExonumNetwork` object with the running node.

    Example:

    >>> network = run_dev_node("exonum-cryptocurrency-advanced")"""
    network = ExonumNetwork(application)

    network.run_dev()

    return network


def run_n_nodes(application: str, nodes_amount: int) -> ExonumNetwork:
    """Creates and runs a network with N validators and return an
    `ExonumNetwork` object with it."""

    address = "127.0.0.1:{}"

    # Assign peer ports starting from 6331.
    available_peer_port = 6331

    # Assign API ports starting from 8080.
    available_api_port = 8080

    network = ExonumNetwork(application)
    network.generate_template(nodes_amount)

    for i in range(nodes_amount):
        network.generate_config(i, address.format(available_peer_port))
        available_peer_port += 1

    for i in range(nodes_amount):
        public_api_address = address.format(available_api_port)
        private_api_address = address.format(available_api_port + 1)
        network.finalize(i, public_api_address, private_api_address)
        available_api_port += 2

    for i in range(nodes_amount):
        network.run_node(i)

    return network


def run_4_nodes(application: str) -> ExonumNetwork:
    """Creates and runs a network with 4 validators and return an
    `ExonumNetwork` object with it.

    Example:

    >>> network = run_4_nodes("exonum-cryptocurrency-advanced")
    >>> for i in range(1, network.validators_count()):
    ...     print(network.api_address(i))
    ...
    '127.0.0.1', 8080, 8081
    '127.0.0.1', 8082, 8083
    '127.0.0.1', 8084, 8085
    '127.0.0.1', 8086, 8087
    """
    return run_n_nodes(application, 4)


def assert_processes_exited_successfully(
    test: unittest.TestCase, outputs: List[ProcessOutput]
) -> None:
    """Asserts that all the processes exited successfully."""
    for output in outputs:
        test.assertEqual(output.exit_result, ProcessExitResult.Ok)
        test.assertEqual(
            output.exit_code, 0, f"Process exited with non-zero code: {output.stderr}"
        )


def launcher_networks(network: ExonumNetwork) -> List[Dict[str, Any]]:
    """Builds a network configuration for `exonum-launcher` from the
    `ExonumNetwork` object."""
    networks = []
    for validator_id in range(network.validators_count()):
        host, public_port, private_port = network.api_address(validator_id)
        node_network = {
            "host": host,
            "ssl": False,
            "public-api-port": public_port,
            "private-api-port": private_port,
        }
        networks.append(node_network)

    # Temporary workaround: supervisor works in simple mode and we need only one node.
    return networks[:1]


def wait_network_to_start(network: ExonumNetwork) -> None:
    """Wait for network starting"""
    wait_api_to_start(network)
    wait_for_block(network, 1)


def wait_for_block(network: ExonumNetwork, height: int = 1) -> None:
    """Wait for block at specific height"""
    for validator_id in range(network.validators_count()):
        host, public_port, private_port = network.api_address(validator_id)
        client = ExonumClient(host, public_port, private_port)
        for _ in range(RETRIES_AMOUNT):
            if client.public_api.get_block(height).status_code == 200:
                break
            time.sleep(0.5)


def wait_api_to_start(network: ExonumNetwork) -> None:
    """Wait for api starting"""
    for validator_id in range(network.validators_count()):
        host, public_port, private_port = network.api_address(validator_id)
        client = ExonumClient(host, public_port, private_port)
        for _ in range(RETRIES_AMOUNT):
            try:
                client.public_api.health_info()
                break
            except ConnectionError:
                time.sleep(0.5)


def generate_config(
    network: ExonumNetwork,
    deadline_height: int = 10000,
    consensus: dict = None,
    artifact_name: str = ARTIFACT_NAME,
    instances: dict = None,
) -> dict:
    if instances is None:
        instances = {}
    cryptocurrency_advanced_config_dict = {
        "networks": launcher_networks(network),
        "deadline_height": deadline_height,
        "consensus": consensus,
        "artifacts": {
            "cryptocurrency": {
                "runtime": "rust",
                "name": artifact_name,
                "version": ARTIFACT_VERSION,
            }
        },
        "instances": instances,
    }

    return cryptocurrency_advanced_config_dict


def find_service_status(available_service, service_name):
    for service in available_service["services"]:
        if service["spec"]["name"] == service_name:
            return service["status"]["type"]
    raise RuntimeError
