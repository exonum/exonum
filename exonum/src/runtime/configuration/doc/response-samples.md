### Public endpoints' response samples

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/actual>

    ```json
    {
      "hash": "b206363922a4b5eda51e4f3d1ef64752e82cb71d780082a461f0b22fe8fca40f",
      "config": {
          "previous_cfg_hash": "872ebf5d638db401326d754ae639b2ae41087569b84933e2cd06e47e859299ca",
          "actual_from": 15,
          "validator_keys": [{
              "consensus_key": "9f6d3fef0fc046efd867b3e9a5392b3bd80b292fbaa1c75d611e47b6c029a53b",
              "service_key": "d496d9ff57fc257950321f130d5b8e4815c9042acb8403bad1a4af7697d937a8"
          }, {
              "consensus_key": "dac373e87491eacad1cd6144c7973a270d7bbd679fede0e3504849879d3b57c0",
              "service_key": "b4b3f56717af7e3f44cea9e45b5335e1dcc5543de2337e7ac009b3a37760c714"
          }],
          "consensus": {
              "first_round_timeout": 3000,
              "status_timeout": 5000,
              "peers_timeout": 10000,
              "txs_block_limit": 1000,
              "max_message_len": 1048576,
              "min_propose_timeout": 10,
              "max_propose_timeout": 600,
              "propose_timeout_threshold": 500
          },
          "services": {
              "configuration": {
                  "majority_count": null
              },
              "cryptocurrency": null
          }
      },
      "propose": "4243dff344336aa411239f388cad1052798234388656f9b1e0716a7dea4e7a11",
      "votes": [{
          "vote_type": "yea",
          "tx_hash": "b1acd3ba7e152645db77d59b0648186dd3d4ddc22882020b4b660898bf1a1041"
      }, {
          "vote_type": "yea",
          "tx_hash": "b5089fe219c59c1674eb01f794b31fb7d7cc519b8d16821cda2ec45132a709d4"
      }]
    }
    ```

2. <http://127.0.0.1:8000/api/services/configuration/v1/configs/following> -
   format same as for `actual`

3. <http://127.0.0.1:8000/api/services/configuration/v1/configs?hash=b206363922a4b5eda51e4f3d1ef64752e82cb71d780082a461f0b22fe8fca40f>

    ```json
    {
      "committed_config": {
          "previous_cfg_hash": "872ebf5d638db401326d754ae639b2ae41087569b84933e2cd06e47e859299ca",
          "actual_from": 15,
          "validator_keys": [{
              "consensus_key": "9f6d3fef0fc046efd867b3e9a5392b3bd80b292fbaa1c75d611e47b6c029a53b",
              "service_key": "d496d9ff57fc257950321f130d5b8e4815c9042acb8403bad1a4af7697d937a8"
          }, {
              "consensus_key": "dac373e87491eacad1cd6144c7973a270d7bbd679fede0e3504849879d3b57c0",
              "service_key": "b4b3f56717af7e3f44cea9e45b5335e1dcc5543de2337e7ac009b3a37760c714"
          }],
          "consensus": {
              "first_round_timeout": 3000,
              "status_timeout": 5000,
              "peers_timeout": 10000,
              "txs_block_limit": 1000,
              "max_message_len": 1048576,
              "min_propose_timeout": 10,
              "max_propose_timeout": 600,
              "propose_timeout_threshold": 500
          },
          "services": {
              "configuration": {
                  "majority_count": null
              },
              "cryptocurrency": null
          }
      },
      "propose": {
          "tx_propose": {
              "cfg": "{\"previous_cfg_hash\":\"872ebf5d638db401326d754ae639b2ae41087569b84933e2cd06e47e859299ca\",\"actual_from\":15,\"validator_keys\":[{\"consensus_key\":\"9f6d3fef0fc046efd867b3e9a5392b3bd80b292fbaa1c75d611e47b6c029a53b\",\"service_key\":\"d496d9ff57fc257950321f130d5b8e4815c9042acb8403bad1a4af7697d937a8\"},{\"consensus_key\":\"dac373e87491eacad1cd6144c7973a270d7bbd679fede0e3504849879d3b57c0\",\"service_key\":\"b4b3f56717af7e3f44cea9e45b5335e1dcc5543de2337e7ac009b3a37760c714\"}],\"consensus\":{\"first_round_timeout\":3000,\"status_timeout\":5000,\"peers_timeout\":10000,\"txs_block_limit\":1000,\"max_message_len\":1048576,\"min_propose_timeout\":10,\"max_propose_timeout\":600,\"propose_timeout_threshold\":500},\"services\":{\"configuration\":{\"majority_count\":null},\"cryptocurrency\":null}}"
          },
          "votes_history_hash": "de9aab8a8e4a5621ea055cd179257cdcb794094fe52fc1f0adf9fe728ab0e63a",
          "num_validators": 2
      }
    }
    ```

4. <http://127.0.0.1:8000/api/services/configuration/v1/configs/votes?hash=b206363922a4b5eda51e4f3d1ef64752e82cb71d780082a461f0b22fe8fca40f>

    ```json
    [
      {
        "vote_type": "yea",
        "tx_hash": "b1acd3ba7e152645db77d59b0648186dd3d4ddc22882020b4b660898bf1a1041"
      }, {
        "vote_type": "yea",
        "tx_hash": "b5089fe219c59c1674eb01f794b31fb7d7cc519b8d16821cda2ec45132a709d4"
      }
    ]
    ```

5. <http://127.0.0.1:8000/api/services/configuration/v1/configs/committed>

    ```json
    [
      {
        "hash": "872ebf5d638db401326d754ae639b2ae41087569b84933e2cd06e47e859299ca",
        "config": {
            "previous_cfg_hash": "0000000000000000000000000000000000000000000000000000000000000000",
            "actual_from": 0,
            "validator_keys": [{
                "consensus_key": "9f6d3fef0fc046efd867b3e9a5392b3bd80b292fbaa1c75d611e47b6c029a53b",
                "service_key": "d496d9ff57fc257950321f130d5b8e4815c9042acb8403bad1a4af7697d937a8"
            }, {
                "consensus_key": "dac373e87491eacad1cd6144c7973a270d7bbd679fede0e3504849879d3b57c0",
                "service_key": "b4b3f56717af7e3f44cea9e45b5335e1dcc5543de2337e7ac009b3a37760c714"
            }],
            "consensus": {
                "first_round_timeout": 3000,
                "status_timeout": 5000,
                "peers_timeout": 10000,
                "txs_block_limit": 1000,
                "max_message_len": 1048576,
                "min_propose_timeout": 10,
                "max_propose_timeout": 200,
                "propose_timeout_threshold": 500
            },
            "services": {
                "configuration": {
                    "majority_count": null
                },
                "cryptocurrency": null
            }
        },
        "propose": null,
        "votes": null
      }, {
        "hash": "b206363922a4b5eda51e4f3d1ef64752e82cb71d780082a461f0b22fe8fca40f",
        "config": {
            "previous_cfg_hash": "872ebf5d638db401326d754ae639b2ae41087569b84933e2cd06e47e859299ca",
            "actual_from": 15,
            "validator_keys": [{
                "consensus_key": "9f6d3fef0fc046efd867b3e9a5392b3bd80b292fbaa1c75d611e47b6c029a53b",
                "service_key": "d496d9ff57fc257950321f130d5b8e4815c9042acb8403bad1a4af7697d937a8"
            }, {
                "consensus_key": "dac373e87491eacad1cd6144c7973a270d7bbd679fede0e3504849879d3b57c0",
                "service_key": "b4b3f56717af7e3f44cea9e45b5335e1dcc5543de2337e7ac009b3a37760c714"
            }],
            "consensus": {
                "first_round_timeout": 3000,
                "status_timeout": 5000,
                "peers_timeout": 10000,
                "txs_block_limit": 1000,
                "max_message_len": 1048576,
                "min_propose_timeout": 10,
                "max_propose_timeout": 600,
                "propose_timeout_threshold": 500
            },
            "services": {
                "configuration": {
                    "majority_count": null
                },
                "cryptocurrency": null
            }
        },
        "propose": "4243dff344336aa411239f388cad1052798234388656f9b1e0716a7dea4e7a11",
        "votes": [{
            "vote_type": "yea",
            "tx_hash": "b1acd3ba7e152645db77d59b0648186dd3d4ddc22882020b4b660898bf1a1041"
        }, {
            "vote_type": "yea",
            "tx_hash": "b5089fe219c59c1674eb01f794b31fb7d7cc519b8d16821cda2ec45132a709d4"
          }
        ]
      }
    ]
    ```

6. <http://127.0.0.1:8000/api/services/configuration/v1/configs/proposed?previous_cfg_hash=872ebf5d638db401326d754ae639b2ae41087569b84933e2cd06e47e859299ca&actual_from=0>

    ```json
    [
      {
        "hash": "b206363922a4b5eda51e4f3d1ef64752e82cb71d780082a461f0b22fe8fca40f",
        "propose_data": {
            "tx_propose": {
                "cfg": "{\"previous_cfg_hash\":\"872ebf5d638db401326d754ae639b2ae41087569b84933e2cd06e47e859299ca\",\"actual_from\":15,\"validator_keys\":[{\"consensus_key\":\"9f6d3fef0fc046efd867b3e9a5392b3bd80b292fbaa1c75d611e47b6c029a53b\",\"service_key\":\"d496d9ff57fc257950321f130d5b8e4815c9042acb8403bad1a4af7697d937a8\"},{\"consensus_key\":\"dac373e87491eacad1cd6144c7973a270d7bbd679fede0e3504849879d3b57c0\",\"service_key\":\"b4b3f56717af7e3f44cea9e45b5335e1dcc5543de2337e7ac009b3a37760c714\"}],\"consensus\":{\"first_round_timeout\":3000,\"status_timeout\":5000,\"peers_timeout\":10000,\"txs_block_limit\":1000,\"max_message_len\":1048576,\"min_propose_timeout\":10,\"max_propose_timeout\":600,\"propose_timeout_threshold\":500},\"services\":{\"configuration\":{\"majority_count\":null},\"cryptocurrency\":null}}"
            },
            "votes_history_hash": "de9aab8a8e4a5621ea055cd179257cdcb794094fe52fc1f0adf9fe728ab0e63a",
            "num_validators": 2
        }
      }
    ]
    ```

### Private endpoints' response samples

1. <http://127.0.0.1:8010/api/services/configuration/v1/configs/postpropose>

    ```bash
    curl -H "Content-type: application/json" -d '{
      "previous_cfg_hash": "b206363922a4b5eda51e4f3d1ef64752e82cb71d780082a461f0b22fe8fca40f",
      "actual_from": 15,
      "validator_keys": [{
          "consensus_key": "9f6d3fef0fc046efd867b3e9a5392b3bd80b292fbaa1c75d611e47b6c029a53b",
          "service_key": "d496d9ff57fc257950321f130d5b8e4815c9042acb8403bad1a4af7697d937a8"
      }, {
          "consensus_key": "dac373e87491eacad1cd6144c7973a270d7bbd679fede0e3504849879d3b57c0",
          "service_key": "b4b3f56717af7e3f44cea9e45b5335e1dcc5543de2337e7ac009b3a37760c714"
      }],
      "consensus": {
          "first_round_timeout": 3000,
          "status_timeout": 5000,
          "peers_timeout": 10000,
          "txs_block_limit": 1000,
          "max_message_len": 1048576,
          "min_propose_timeout": 10,
          "max_propose_timeout": 600,
          "propose_timeout_threshold": 500
      },
      "services": {
          "configuration": {
              "majority_count": null
          },
          "cryptocurrency": null
      }
    }'  http://127.0.0.1:8010/api/v1/configs/postpropose
    ```
    ```json
    {
      "cfg_hash": "f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a",
      "tx_hash": "b833617f45e79a785335fdfff161e976f7fa524cd69791fda79344d20f882060"
    }
    ```

2. <http://127.0.0.1:8011/api/services/configuration/v1/configs/postvote>

    ```bash
    curl -H "Content-type: application/json" -d '{
        "hash": "f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a"
      }' http://127.0.0.1:8011/api/services/configuration/v1/configs/postvote
    ```
    ```javascript
    {
      "tx_hash": "3e057783ff4bcb5f180625838d6cfb6317a161ea7024fb35372c1ce722dfc066"
    }
    ```

3. <http://127.0.0.1:8012/api/services/configuration/v1/configs/postagainst>

    ```bash
    curl -H "Content-type: application/json" -d '{
        "hash": "f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a"
      }' http://127.0.0.1:8012/api/services/configuration/v1/configs/postagainst
    ```
    ```javascript
    {
      "tx_hash": "3e057783ff4bcb5f180625838d6cfb6317a161ea7024fb35372c1ce722dfc066"
    }
    ```
