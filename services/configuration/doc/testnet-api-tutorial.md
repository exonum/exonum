# Configuration Service Tutorial

The present tutorial provides instructions on how to configure an Exonum
blockchain with the help of the Configuration Service.

## Running a Testnet

### Using Docker

<!-- spell-checker:ignore vitvakatu -->

To automatically configure and launch a testnet with 4 nodes simply run the
following command:

```bash
docker run -p 8000-8007:8000-8007 \
exonumhub/exonum-configuration-service:example 4
```

Docker will automatically pull an image from the repository and run 4 nodes with
public endpoints at `127.0.0.1:8000`, ..., `127.0.0.1:8003` and
private ones at `127.0.0.1:8004`, ..., `127.0.0.1:8007`.

You can also use helper [script](../docker/example-start.sh):

```bash
./docker/example-start.sh <number of nodes>
```

To stop the docker container, use `docker stop <container id>` command.

### Manually

#### Build an Example Binary

To build an [example binary](../examples/configuration.rs) of the Exonum
blockchain manually and mount a single configuration service on it, run the
following command:

```bash
git clone https://github.com/exonum/exonum

cd exonum/services/configuration

cargo install --example configuration --path .
```

- `--example` is a name of the mounted service
- `--path` is a route to the service configuration files.

You can find information on the required `exonum` crate system dependencies and
Rust toolchain configuration in the
[Exonum Installation Guide](https://exonum.com/doc/version/latest/get-started/install/).

#### Generate Configs

To generate a project template that will be applied by 4 validators run the
following command:

```sh
mkdir example
configuration generate-template example/common.toml --validators-count 4
```

- `--validators-count` is a number of validators in the network.

To generate the templates of the configurations of the nodes, do the following:
<!-- markdownlint-disable MD013 -->

```sh
configuration generate-config example/common.toml  example/pub_1.toml example/sec_1.toml --peer-address 127.0.0.1:6331 -c example/consensus_1.toml -s example/service_1.toml -n

configuration generate-config example/common.toml  example/pub_2.toml example/sec_2.toml --peer-address 127.0.0.1:6332 -c example/consensus_2.toml -s example/service_2.toml -n

configuration generate-config example/common.toml  example/pub_3.toml example/sec_3.toml --peer-address 127.0.0.1:6333 -c example/consensus_3.toml -s example/service_3.toml -n

configuration generate-config example/common.toml  example/pub_4.toml example/sec_4.toml --peer-address 127.0.0.1:6334 -c example/consensus_4.toml -s example/service_4.toml -n
```

- `--peer-address` is an address of the current node used by other peers to
  connect to each other.

Note that in case of copying files with consensus and service keys to the other machines, you must change the access permissions of these files for every machine.
For example:

```sh
sudo chmod 600 consensus_1.toml
sudo chmod 600 service_1.toml
```

The command below will finalize generation of the configurations of the nodes:

```sh
configuration finalize --public-api-address 0.0.0.0:8200 --private-api-address 0.0.0.0:8091 example/sec_1.toml example/node_1_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

configuration finalize --public-api-address 0.0.0.0:8201 --private-api-address 0.0.0.0:8092 example/sec_2.toml example/node_2_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

configuration finalize --public-api-address 0.0.0.0:8202 --private-api-address 0.0.0.0:8093 example/sec_3.toml example/node_3_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

configuration finalize --public-api-address 0.0.0.0:8203 --private-api-address 0.0.0.0:8094 example/sec_4.toml example/node_4_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml
```

- `--public-api-address` is for Exonum [public HTTP API endpoints](#public-endpoints)
- `--private-api-address` is for Exonum [private HTTP API endpoints](#private-endpoints)
- `--public-configs` is a list of files with public configs of all the network
  nodes.

#### Run Nodes

To run the network, use the following commands:

```sh
configuration run --node-config example/node_1_cfg.toml --db-path example/db1 --public-api-address 0.0.0.0:8200 --consensus-key-pass pass --service-key-pass pass

configuration run --node-config example/node_2_cfg.toml --db-path example/db2 --public-api-address 0.0.0.0:8201 --consensus-key-pass pass --service-key-pass pass

configuration run --node-config example/node_3_cfg.toml --db-path example/db3 --public-api-address 0.0.0.0:8202 --consensus-key-pass pass --service-key-pass pass

configuration run --node-config example/node_4_cfg.toml --db-path example/db4 --public-api-address 0.0.0.0:8203 --consensus-key-pass pass --service-key-pass pass
```

<!-- markdownlint-enable MD013 -->

- `--node-config` is a path to the node configuration
- `--db-path` is a path to the database
- `--consensus-key-pass` is a password to the file with the consensus key of the
  node
- `--service-key-pass` is a password to the file with the service key of the
  node

## Configuration Service REST API

Configuration Service allows modifying the [global configuration][system-configuration]
by the means of proposing a new configuration and voting for proposed
configurations among the validators.

See the detailed description of the business logic behind the service in our
[documentation][configuration-updater].

The service operates via REST API and provides a set of public and private
endpoints. The mentioned [documentation][rest-api] will provide you with the
full list of available endpoints, applied data types and examples of responses.

[configuration-updater]: https://exonum.com/doc/version/latest/advanced/configuration-updater/
[system-configuration]: https://exonum.com/doc/version/latest/architecture/configuration/
[rest-api]: https://exonum.com/doc/version/latest/advanced/configuration-updater/#rest-api
