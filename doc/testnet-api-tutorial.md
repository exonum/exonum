# Configuration service tutorial

## Build instructions

To build an [example binary](examples/configuration.rs) of exonum blockchain 
with the single configuration service mounted, run:

```bash
cargo install --example configuration
```

`exonum` crate system dependencies and rust toolchain configuration - 
[exonum install instructions](https://github.com/exonum/exonum-core/blob/master/INSTALL.md).

## Running testnet

1. Generate testnet dir and testnet config.

  - 4 is a required indexed parameter and stands for number of nodes in testnet:

    ```bash
    mkdir -p testnet/configuration_service
    cd testnet/configuration_service
    configuration generate --output-dir . --start-port 5400 4
    cd ..
    ```

  - This should create following config for testnet:

    ```bash
    $ tree configuration_service/
    configuration_service/
    └── validators
      ├── 0.toml
      ├── 1.toml
      ├── 2.toml
      └── 3.toml
    ```

2. Run 4 nodes:

  - manually for each node process:

    ```bash
    configuration run --node-config configuration_service/validators/0.toml --leveldb-path configuration_service/db/0 --public-port 8000 --private-port 8010
    configuration run --node-config configuration_service/validators/1.toml --leveldb-path configuration_service/db/1 --public-port 8001 --private-port 8011
    configuration run --node-config configuration_service/validators/2.toml --leveldb-path configuration_service/db/2 --public-port 8002 --private-port 8012
    configuration run --node-config configuration_service/validators/3.toml --leveldb-path configuration_service/db/3 --public-port 8003 --private-port 8013
    ```

      - parameters

          - `--public-port` is for configuration service's [public http api 
          endpoints](#public-endpoints)

          - `--private-port` is for configuration service's private http api 
          endpoints

          - `--node-config` and `--leveldb-path` are described on 
          [exonum install instructions](https://github.com/exonum/exonum-core/blob/master/INSTALL.md)

  - automatically via the [supervisord](http://supervisord.org/) utility.

     1. set the **TESTNET_DESTDIR** environment variable to the `testnet` dir 
        created above:

        ```bash
        $ pwd
        /Users/user/Exonum/testnet
        $ export TESTNET_DESTDIR=/Users/user/Exonum/testnet
        ```

     1. run [helper script](../testnet/testnetctl.sh) for initializing 
        `supervisor` and `configuration_service` process group 
        [config](../testnet/supervisord) to `$TESTNET_DESDIR` directory.
        
        ```bash
        ./testnet/testnetctl.sh enable
        ```

     1. go to `$TESTNET_DESTDIR`. It contains new `etc`, `run`, `log`folders.

        ```bash
        $ cd $TESTNET_DESTDIR
        $ tree .
        .
        ├── configuration_service
        │   └── validators
        │       ├── 0.toml
        │       ├── 1.toml
        │       ├── 2.toml
        │       └── 3.toml
        ├── etc
        │   ├── conf.d
        │   │   └── configuration_service.conf
        │   └── supervisord.conf
        ├── log
        │   ├── supervisor
        │   │   ├── configuration_service_00-stderr---supervisor-rMqmIy.log
        │   │   ├── configuration_service_00-stdout---supervisor-BWDVqD.log
        │   │   ├── configuration_service_01-stderr---supervisor-2NgHVG.log
        │   │   ├── configuration_service_01-stdout---supervisor-Qcyr2v.log
        │   │   ├── configuration_service_02-stderr---supervisor-olCKCx.log
        │   │   ├── configuration_service_02-stdout---supervisor-F2IdNB.log
        │   │   ├── configuration_service_03-stderr---supervisor-Z3MxXS.log
        │   │   └── configuration_service_03-stdout---supervisor-s29Fd_.log
        │   └── supervisord.log
        └── run
            └── supervisord.pid

        7 directories, 16 files
        ```

     1. launch `configuration_service` process group.

        ```bash
        $ supervisorctl start configuration_service:*
        configuration_service:configuration_service_01: started
        configuration_service:configuration_service_00: started
        configuration_service:configuration_service_03: started
        configuration_service:configuration_service_02: started
        ```


## Global variable service http api

### Public endpoints

1. `http://127.0.0.1:8000/api/v1/config/actual` - посмотреть актуальный конфиг и его хэш. Это важно, чтобы указать в следуещем конфиге ссылку на предыдущий в поле `"previous_cfg_hash": "7aa8c6a39438443ccc2a31d28cbd6b3f897be9db3ff13711b85a3eddb5eebcba"`

`http://127.0.0.1:8000/api/v1/config/following` - посмотреть принятый, но еще не вступивший в силу следующий конфиг, и его хэш.

`http://127.0.0.1:8000/api/v1/configs/7aa8c6a39438443ccc2a31d28cbd6b3f897be9db3ff13711b85a3eddb5eebcba` - посмотреть данные о конфиге по его хэшу

`http://127.0.0.1:8000/api/v1/configs/7aa8c6a39438443ccc2a31d28cbd6b3f897be9db3ff13711b85a3eddb5eebcba/votes` - посмотреть принятые голоса

1. внести предложение за новый конфиг:

  ```
  curl -X POST -d '{
     "actual_from": 1000,
     "consensus": {
             "peers_timeout": 10000,
             "propose_timeout": 500,
             "round_timeout": 3000,
             "status_timeout": 5000,
             "txs_block_limit": 1000
     },
     "previous_cfg_hash": "7aa8c6a39438443ccc2a31d28cbd6b3f897be9db3ff13711b85a3eddb5eebcba",
     "services": {
             "1": {"config_service_str": 56578734},
             "128": {"crypto_currency_svc_str": 56578734}
     },
     "validators": [
             "8cb9cb5b4ab79f9ca35bc12bb45d72e9e3af4fc883fd95fcde9703dcb74d50d2",
             "d35e90e6745775d678485c6a6c835e02e4419959fd50a5d2ee22c81d8f2cc139",
            "1f53843a6b893a25a068901309a1f84d32a5f89e5027d7b9b503f9d73ddeb070",
             "6efe808cd598397d4ccae314c9d184201736055448a5ae766696f7546b1a3e28"
     ]
  }' http://127.0.0.1:8010/api/v1/configs/postpropose
  ```

В ответ на curl придет ответ вида

```
{ cfg_hash: "…", tx_hash: "…"}
```

по cfg_hash - можно смотреть конфиг статус пропоуза `http://127.0.0.1:8000/api/v1/configs/cfg_hash`

1. вот так вот проголосовать за один и тот же конфиг разными валидаторами по его cfg_hash `curl -X POST -d '{}' http://127.0.0.1:8010/api/v1/configs/a0803af8d614ab7484ebc3cf605adfb1a600161d905bfa7fb75ca26a543d0e0f/postvote curl -X POST -d '{}' http://127.0.0.1:8011/api/v1/configs/a0803af8d614ab7484ebc3cf605adfb1a600161d905bfa7fb75ca26a543d0e0f/postvote curl -X POST -d '{}' http://127.0.0.1:8012/api/v1/configs/a0803af8d614ab7484ebc3cf605adfb1a600161d905bfa7fb75ca26a543d0e0f/postvote`
