"""Library for writing integration tests for Exonum."""
import os
from typing import List, Dict, Tuple, Optional, Any

import requests

from suite.temp_dir import TempDir
from suite.process_manager import ProcessManager, ProcessOutput

# Time to wait for node to shutdown in seconds.
# Actix can be really slow while joining its threads.
NODE_SHUTDOWN_TIMEOUT = 100.0


class ExonumNetwork:
    """Class representing runner of the Exonum network.
    It provides an interface to start a network of N validatos in the separate threads.

    Application is expected to be installed and referenced by its name."""

    def __init__(self, app_name: str) -> None:
        self._app_dir = TempDir.create()
        self._app_name = app_name
        self._processes: List[ProcessManager] = []
        self._validators_count = 0
        self._private_api_addresses: Dict[int, str] = dict()
        self._public_api_addresses: Dict[int, str] = dict()

    def __enter__(self) -> "ExonumNetwork":
        return self

    def __exit__(self, exc_type: Optional[type], exc_value: Optional[Any], exc_traceback: Optional[object]) -> None:
        # Cleanup after use.
        self._app_dir.remove()

    def generate_template(self, validators_count: int, supervisor_mode: str = "simple") -> None:
        """Runs `generate-template` command."""
        common_config = self._common_config()

        # {exonum-app} generate-template example/common.toml \
        #   --validators-count 4 --supervisor-mode simple
        args = [common_config]
        args.append(f"--validators-count {validators_count}")
        args.append(f"--supervisor-mode {supervisor_mode}")

        self._run_command("generate-template", args)

        self._validators_count = validators_count

    def generate_config(self, validator_id: int, peer_address: str) -> None:
        """Runs `generate-config` command."""
        common_config = self._common_config()
        validator_config = self._validator_config_dir(validator_id)

        # {exonum_app} generate-config example/common.toml  example/1 --peer-address 127.0.0.1:6331 -n
        args = [common_config]
        args.append(validator_config)
        args.append(f"--peer-address {peer_address}")
        args.append("-n")

        self._run_command("generate-config", args)

    def finalize(self, validator_id: int, public_api_address: str, private_api_address: str) -> None:
        """Runs `finalize` command."""

        sec_config = self._validator_config(validator_id, "sec")
        node_config = self._validator_config(validator_id, "node")
        public_configs = self._public_configs()

        # {exonum_app} finalize --public-api-address 0.0.0.0:8200 --private-api-address 0.0.0.0:8091 \
        # example/1/sec.toml example/1/node.toml --public-configs example/{1,2,3,4}/pub.toml
        args = [f"--public-api-address {public_api_address}"]
        args.append(f"--private-api-address {private_api_address}")
        args.append(sec_config)
        args.append(node_config)
        args.append(f"--public-configs {public_configs}")

        self._run_command("finalize", args)

        # Store API addresses.
        self._public_api_addresses[validator_id] = public_api_address
        self._private_api_addresses[validator_id] = private_api_address

    def run_node(self, validator_id: int) -> None:
        """Runs the node."""

        node_config = self._validator_config(validator_id, "node")
        db_path = self._db_path(validator_id)

        # {exonum_app} run --node-config example/1/node.toml --db-path example/1/db \
        # --public-api-address 0.0.0.0:8200 --master-key-pass pass
        args = [f"--node-config {node_config}"]
        args.append(f"--db-path {db_path}")
        args.append(f"--public-api-address {self._public_api_addresses[validator_id]}")
        args.append(f"--master-key-pass pass")

        command = f"{self._app_name} run " + " ".join(args)

        # Run the node in the separate thread.

        process = ProcessManager(command)
        process.start()

        self._processes.append(process)

    def run_dev(self) -> None:
        """Runs the node in the `run-dev` mode."""
        command = f"{self._app_name} run-dev -a {self._app_dir.path()}"

        process = ProcessManager(command)
        process.start()

        self._processes.append(process)

        # Init metainfo, so you can work with run-dev node the same way
        # as you'll work with a properly initialized network.
        self._validators_count = 1
        self._public_api_addresses[0] = "127.0.0.1:8080"
        self._private_api_addresses[0] = "127.0.0.1:8081"

    def stop(self) -> List[ProcessOutput]:
        """Stops all the nodes and return outputs of each process."""

        # Send shutdown requests (it should contain word `null` in the request body).
        shutdown_endpoint = "http://{}/api/system/v1/shutdown"
        for private_address in self._private_api_addresses.values():
            url = shutdown_endpoint.format(private_address)
            data = "null"
            requests.post(url, data=data, headers={"content-type": "application/json"})

        # Join every process and collect outputs.
        outputs = []
        kill_timeout = NODE_SHUTDOWN_TIMEOUT
        for process in self._processes:
            output = process.join_process(kill_timeout)
            outputs.append(output)

        return outputs

    def validators_count(self) -> int:
        """Returns amount of validators in network."""
        return self._validators_count

    def api_address(self, validator_id: int) -> Tuple[str, int, int]:
        """Returns a tuple of (host, public port, private port)."""
        if 0 <= validator_id < self._validators_count:
            public = self._public_api_addresses[validator_id]
            private = self._private_api_addresses[validator_id]

            host, public_port = public.split(":")
            private_port = private.split(":")[1]

            return host, int(public_port), int(private_port)

        raise RuntimeError(f"Incorrect node ID, expected >= 0 and < {self._validators_count}, got {validator_id}")

    def _common_config(self) -> str:
        return os.path.join(self._app_dir.path(), "common.toml")

    def _validator_config_dir(self, validator_id: int) -> str:
        return os.path.join(self._app_dir.path(), str(validator_id))

    def _validator_config(self, validator_id: int, config: str) -> str:
        return os.path.join(self._validator_config_dir(validator_id), f"{config}.toml")

    def _db_path(self, validator_id: int) -> str:
        return os.path.join(self._validator_config_dir(validator_id), "db")

    def _validator_ids(self) -> str:
        # 4 -> "0,1,2,3"
        return ",".join(map(str, range(self._validators_count)))

    def _public_configs(self) -> str:
        configs = []
        for i in range(self.validators_count()):
            config = os.path.join(self._app_dir.path(), str(i), "pub.toml")
            configs.append(config)
        return " ".join(configs)

    def _run_command(self, command_name: str, args: List[str]) -> None:
        command = self._command(command_name, args)
        process = ProcessManager(command)

        output = process.run_sync()

        if output.exit_code != 0:
            error = f"""Command `{command}` exited with non-zero code {output.exit_code}
            stdout: {output.stdout}
            stderr: {output.stderr}"""
            raise RuntimeError(error)

    def _command(self, command: str, args: List[str]) -> str:
        command = f"{self._app_name} {command} " + " ".join(args)
        return command
