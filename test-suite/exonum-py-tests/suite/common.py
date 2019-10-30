"""Module containing common scenarios that can be used
for writing tests with less boiler-plate."""

from typing import List, Dict, Any
import unittest

from suite import ExonumNetwork, ProcessOutput, ProcessExitResult


def run_dev_node(application: str) -> ExonumNetwork:
    """Starts a single node in the run-dev mode and returns
    `ExonumNetwork object with the running node.

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
    ...     print(network.api_address(i)
    ...
    '127.0.0.1', 8080, 8081
    '127.0.0.1', 8082, 8083
    '127.0.0.1', 8084, 8085
    '127.0.0.1', 8086, 8087
    """
    return run_n_nodes(application, 4)


def assert_processes_exited_successfully(test: unittest.TestCase, outputs: List[ProcessOutput]) -> None:
    """Asserts that all the processes exited successfully."""
    for output in outputs:
        test.assertEqual(output.exit_result, ProcessExitResult.Ok)
        test.assertEqual(output.exit_code, 0, f"Process exited with non-zero code: {output.stderr}")


def launcher_networks(network: ExonumNetwork) -> List[Dict[str, Any]]:
    """Builds a network configuration for `exonum-launcher` from the
    `ExonumNetwork` object."""
    networks = []
    for validator_id in range(network.validators_count()):
        host, public_port, private_port = network.api_address(validator_id)
        node_network = {"host": host, "ssl": False, "public-api-port": public_port, "private-api-port": private_port}
        networks.append(node_network)

    return networks
