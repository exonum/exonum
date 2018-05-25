### Public endpoints' response samples

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/actual>

    ```json
    {
      "config": {
        "actual_from": 13000,
        "consensus": {
          "max_message_len": 1048576,
          "max_propose_timeout": 1000,
          "min_propose_timeout": 100,
          "peers_timeout": 10000,
          "propose_timeout_threshold": 99,
          "round_timeout": 3000,
          "status_timeout": 5000,
          "txs_block_limit": 100
        },
        "majority_count": null,
        "previous_cfg_hash": "b5273e3b5180db238f51d8317b27daac29d1bc162e0b75294e5e0b27677d3242",
        "services": {
          "configuration": null,
          "cryptocurrency": null
        },
        "validator_keys": [
          {
            "consensus_key": "0af291925e899454cbc7fb5a258371ef48dc6b33f5f66281d40bf73d43b1f78f",
            "service_key": "8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d"
          },
          {
            "consensus_key": "eef70a7473051bb2c994a6b21438e786e41e33fc97c82394c7e5d1221656bf2c",
            "service_key": "7ed8d1b21f6d44b09cd3062526eeeca1495338c37f4d3774a46f61ab3849215b"
          }
        ]
      },
      "hash": "9f19a7e3a623336cd49c2a2ac92c5fed172dcbeaf6dcc09515669d44797e7470",
      "propose": "b1e0ff09b2cd2adc48b0c868f85fd24d84e893160836b56730b729201a0c4a86",
      "votes": [
        {
          "body": {
            "cfg_hash": "9f19a7e3a623336cd49c2a2ac92c5fed172dcbeaf6dcc09515669d44797e7470",
            "from": "8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d"
          },
          "message_id": 1,
          "protocol_version": 0,
          "service_id": 1,
          "signature": "ac9cc7a189f34a31ca76cf1756581b0f0d49942ddd99692c2905371d5ee3803f7951baf7cab7f9cbac5064e5e52f0f95fe88bd4410a18d974d1da991a5227800",
          "vote_for": "yea"
        },
        {
          "body": {
            "cfg_hash": "9f19a7e3a623336cd49c2a2ac92c5fed172dcbeaf6dcc09515669d44797e7470",
            "from": "7ed8d1b21f6d44b09cd3062526eeeca1495338c37f4d3774a46f61ab3849215b"
          },
          "message_id": 1,
          "protocol_version": 0,
          "service_id": 1,
          "signature": "091629aabdc8a35742303a34dacaca20e7b67faea7f33d0cb77a0c0db172f0c4096b7e41dd6d65eee28b1b664d904fa6a125d8338ecf9a43c62f456d73f70007",
          "vote_for": "yea"
        }
      ]
    }
    ```

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/following> -
   format same as for `actual`

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/b5273e3b5180db238f51d8317b27daac29d1bc162e0b75294e5e0b27677d3242>

    ```json
    {
      "committed_config": {
        "actual_from": 12000,
        "consensus": {
          "max_message_len": 1048576,
          "max_propose_timeout": 1000,
          "min_propose_timeout": 100,
          "peers_timeout": 10000,
          "propose_timeout_threshold": 100,
          "round_timeout": 3000,
          "status_timeout": 5000,
          "txs_block_limit": 100
        },
        "majority_count": null,
        "previous_cfg_hash": "4fb39c965624edbec7dbf9ddd902662539c594b082ee957ab92614d0867b87e2",
        "services": {
          "configuration": null,
          "cryptocurrency": null
        },
        "validator_keys": [
          {
            "consensus_key": "0af291925e899454cbc7fb5a258371ef48dc6b33f5f66281d40bf73d43b1f78f",
            "service_key": "8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d"
          },
          {
            "consensus_key": "eef70a7473051bb2c994a6b21438e786e41e33fc97c82394c7e5d1221656bf2c",
            "service_key": "7ed8d1b21f6d44b09cd3062526eeeca1495338c37f4d3774a46f61ab3849215b"
          }
        ]
      },
      "propose": {
        "num_validators": "2",
        "tx_propose": {
          "body": {
            "cfg": "{\"previous_cfg_hash\":\"4fb39c965624edbec7dbf9ddd902662539c594b082ee957ab92614d0867b87e2\",\"actual_from\":12000,\"validator_keys\":[{\"consensus_key\":\"0af291925e899454cbc7fb5a258371ef48dc6b33f5f66281d40bf73d43b1f78f\",\"service_key\":\"8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d\"},{\"consensus_key\":\"eef70a7473051bb2c994a6b21438e786e41e33fc97c82394c7e5d1221656bf2c\",\"service_key\":\"7ed8d1b21f6d44b09cd3062526eeeca1495338c37f4d3774a46f61ab3849215b\"}],\"consensus\":{\"round_timeout\":3000,\"status_timeout\":5000,\"peers_timeout\":10000,\"txs_block_limit\":100,\"max_message_len\":1048576,\"min_propose_timeout\":100,\"max_propose_timeout\":1000,\"propose_timeout_threshold\":100},\"majority_count\":null,\"services\":{\"configuration\":null,\"cryptocurrency\":null}}",
            "from": "8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d"
          },
          "message_id": 0,
          "protocol_version": 0,
          "service_id": 1,
          "signature": "1b602c1313df8b5ca71324db0f4faf7aeda65634f9f7474ef006f63c75796e0d76691331d796cc4972543689ea40e8a182c75619cf138dfe0e041c398dc83806"
        },
        "votes_history_hash": "f09c0b220a0f43ee56706a98d721e71f1f07399c1ecb6315b3fe48416d942c88"
      }
    }
    ```

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/b5273e3b5180db238f51d8317b27daac29d1bc162e0b75294e5e0b27677d3242/votes>

    ```json
    [
      {
        "body": {
          "cfg_hash": "b5273e3b5180db238f51d8317b27daac29d1bc162e0b75294e5e0b27677d3242",
          "from": "8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d"
        },
        "message_id": 1,
        "protocol_version": 0,
        "service_id": 1,
        "signature": "15befffad1f5f6a3c6f7195564df1cb0d3deb0fce5acfe10fec75922bd34d029689be57fb1b61277120f40a19ec31345ff15bfa352260f1945d26b2798a9040f",
        "vote_for": "yea"
      },
      {
        "body": {
          "cfg_hash": "b5273e3b5180db238f51d8317b27daac29d1bc162e0b75294e5e0b27677d3242",
          "from": "7ed8d1b21f6d44b09cd3062526eeeca1495338c37f4d3774a46f61ab3849215b"
        },
        "message_id": 1,
        "protocol_version": 0,
        "service_id": 1,
        "signature": "2cc730a12326928162c6bbdb96d515b5aa96b26e29447e8e3c3569aa86e8fd88cffd87d8067f80e4738ce88e59f5dc191613389dcbc83c434e7fb2205f71c307",
        "vote_for": "yea"
      }
    ]
    ```

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/committed>

    ```json
    [
      {
        "config": {
          "actual_from": 0,
          "consensus": {
            "max_message_len": 1048576,
            "max_propose_timeout": 200,
            "min_propose_timeout": 10,
            "peers_timeout": 10000,
            "propose_timeout_threshold": 500,
            "round_timeout": 3000,
            "status_timeout": 5000,
            "txs_block_limit": 1000
          },
          "majority_count": null,
          "previous_cfg_hash": "0000000000000000000000000000000000000000000000000000000000000000",
          "services": {
            "configuration": null,
            "cryptocurrency": null
          },
          "validator_keys": [
            {
              "consensus_key": "0af291925e899454cbc7fb5a258371ef48dc6b33f5f66281d40bf73d43b1f78f",
              "service_key": "8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d"
            },
            {
              "consensus_key": "eef70a7473051bb2c994a6b21438e786e41e33fc97c82394c7e5d1221656bf2c",
              "service_key": "7ed8d1b21f6d44b09cd3062526eeeca1495338c37f4d3774a46f61ab3849215b"
            }
          ]
        },
        "hash": "9e7ec0fc3f5bf8e747b272525bcaf3869fb0b865eb2c196e6c7d95bcb742415e",
        "propose": null,
        "votes": null
      },
      {
        "config": {
          "actual_from": 10000,
          "consensus": {
            "max_message_len": 1048576,
            "max_propose_timeout": 1000,
            "min_propose_timeout": 100,
            "peers_timeout": 10000,
            "propose_timeout_threshold": 100,
            "round_timeout": 3000,
            "status_timeout": 5000,
            "txs_block_limit": 1000
          },
          "majority_count": null,
          "previous_cfg_hash": "9e7ec0fc3f5bf8e747b272525bcaf3869fb0b865eb2c196e6c7d95bcb742415e",
          "services": {
            "configuration": null,
            "cryptocurrency": null
          },
          "validator_keys": [
            {
              "consensus_key": "0af291925e899454cbc7fb5a258371ef48dc6b33f5f66281d40bf73d43b1f78f",
              "service_key": "8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d"
            },
            {
              "consensus_key": "eef70a7473051bb2c994a6b21438e786e41e33fc97c82394c7e5d1221656bf2c",
              "service_key": "7ed8d1b21f6d44b09cd3062526eeeca1495338c37f4d3774a46f61ab3849215b"
            }
          ]
        },
        "hash": "4fb39c965624edbec7dbf9ddd902662539c594b082ee957ab92614d0867b87e2",
        "propose": "f321c485c2c9dcf6cf496b9ec3de0c3784782fe04408ab13de8312cba1a96b1e",
        "votes": [
          {
            "body": {
              "cfg_hash": "4fb39c965624edbec7dbf9ddd902662539c594b082ee957ab92614d0867b87e2",
              "from": "8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d"
            },
            "message_id": 1,
            "protocol_version": 0,
            "service_id": 1,
            "signature": "dde8202292067d7d42fbe3b05132667fcaecfb8328713f4c3c7b5ba252cf36450ac135fc3969b4264cd9c367c725f73a3a64accc4ea9a9519ebda82f5c24c50a",
            "vote_for": "yea"
          },
          {
            "body": {
              "cfg_hash": "4fb39c965624edbec7dbf9ddd902662539c594b082ee957ab92614d0867b87e2",
              "from": "7ed8d1b21f6d44b09cd3062526eeeca1495338c37f4d3774a46f61ab3849215b"
            },
            "message_id": 1,
            "protocol_version": 0,
            "service_id": 1,
            "signature": "352995685ac7681c794fe66496b5af2d75c825dbc5648954859b23f4e4a504f3f75ec7f002f4711ff0b6d81ec1e9e039d3d16646e74c76f25bd259a84fde7d04",
            "vote_for": "yea"
          }
        ]
      }
    ]
    ```

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/proposed?previous_cfg_hash=51e63930260249bf03228de100072c69a37e7693c1b33b3c46c529087fed83eb&actual_from=3100>

    ```json
    [
      {
        "hash": "b5273e3b5180db238f51d8317b27daac29d1bc162e0b75294e5e0b27677d3242",
        "propose_data": {
          "num_validators": "2",
          "tx_propose": {
            "body": {
              "cfg": "{\"previous_cfg_hash\":\"4fb39c965624edbec7dbf9ddd902662539c594b082ee957ab92614d0867b87e2\",\"actual_from\":12000,\"validator_keys\":[{\"consensus_key\":\"0af291925e899454cbc7fb5a258371ef48dc6b33f5f66281d40bf73d43b1f78f\",\"service_key\":\"8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d\"},{\"consensus_key\":\"eef70a7473051bb2c994a6b21438e786e41e33fc97c82394c7e5d1221656bf2c\",\"service_key\":\"7ed8d1b21f6d44b09cd3062526eeeca1495338c37f4d3774a46f61ab3849215b\"}],\"consensus\":{\"round_timeout\":3000,\"status_timeout\":5000,\"peers_timeout\":10000,\"txs_block_limit\":100,\"max_message_len\":1048576,\"min_propose_timeout\":100,\"max_propose_timeout\":1000,\"propose_timeout_threshold\":100},\"majority_count\":null,\"services\":{\"configuration\":null,\"cryptocurrency\":null}}",
              "from": "8b097c60926490c746f39c2fbbc55c10341d1d8160b84c1c4d6c5a777a2a704d"
            },
            "message_id": 0,
            "protocol_version": 0,
            "service_id": 1,
            "signature": "1b602c1313df8b5ca71324db0f4faf7aeda65634f9f7474ef006f63c75796e0d76691331d796cc4972543689ea40e8a182c75619cf138dfe0e041c398dc83806"
          },
          "votes_history_hash": "f09c0b220a0f43ee56706a98d721e71f1f07399c1ecb6315b3fe48416d942c88"
        }
      }
    ]
    ```

### Private endpoints' response samples

1. <http://127.0.0.1:8010/api/services/configuration/v1/configs/postpropose>

    ```bash
    curl -X POST -d '{
          "actual_from": 5500,
          "consensus": {
            "peers_timeout": 10000,
            "propose_timeout": 500,
            "round_timeout": 3000,
            "status_timeout": 5000,
            "timeout_adjuster": {
              "timeout": 500,
              "type": "Constant"
            },
            "txs_block_limit": 1000
          },
          "previous_cfg_hash": "daeb250090f2d6b3689a5effd32cb16a77b7770bb1df123e1f32b13143cd3623",
          "services": {
            "configuration": null
          },
          "validator_keys": [
            {
              "consensus_key": "42fc056f90f60569ec157462ff1fb700afac1c7994cca7f68f549fc00fa0b038",
              "service_key": "48463d7eb0d75eb4e7426790bcec10ecb9de2501aed1c7325cea1155666332b4"
            },
            {
              "consensus_key": "38126e12682609855bfe4bc87fe8b404563148072c3c75438c323053d0dc544c",
              "service_key": "dbe3ae83538cb8482d35e4cf22dcc8d3cbec1979e76925aadaeb79f29a3b896e"
            }
          ]
    }'  http://127.0.0.1:8010/api/v1/configs/postpropose
    ```
    ```json
    {
      "cfg_hash": "f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a",
      "tx_hash": "b833617f45e79a785335fdfff161e976f7fa524cd69791fda79344d20f882060"
    }
    ```

1. <http://127.0.0.1:8011/api/services/configuration/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/postvote>

    ```bash
    curl -X POST -d '{}' http://127.0.0.1:8011/api/services/configuration/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/postvote
    ```
    ```javascript
    {
      "tx_hash": "3e057783ff4bcb5f180625838d6cfb6317a161ea7024fb35372c1ce722dfc066"
    }
    ```

1. <http://127.0.0.1:8012/api/services/configuration/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/postagainst>

    ```bash
    curl -X POST -d '{}' http://127.0.0.1:8012/api/services/configuration/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/postagainst
    ```
    ```javascript
    {
      "tx_hash": "3e057783ff4bcb5f180625838d6cfb6317a161ea7024fb35372c1ce722dfc066"
    }
    ```
