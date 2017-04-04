1) компилишь https://github.com/exonum/exonum-configuration/blob/master/examples/configuration.rs с помощью `cargo install —force —example configuration`

2) стартовый конфиг `configuration generate -o ~/Downloads/test_cfg 4 -p 4400`

3) запуск нод: 
 ```configuration run -c ~/Downloads/test_cfg/validators/0.toml -d ~/Downloads/test_cfg/db/0 -p 8000 -s 8010
configuration run -c ~/Downloads/test_cfg/validators/1.toml -d ~/Downloads/test_cfg/db/1 -p 8001 -s 8011
configuration run -c ~/Downloads/test_cfg/validators/2.toml -d ~/Downloads/test_cfg/db/2 -p 8002 -s 8012
configuration run -c ~/Downloads/test_cfg/validators/3.toml -d ~/Downloads/test_cfg/db/3 -p 8003 -s 8013
```

4) запуск нод. Сорри, что не написал параметры для supervisord. -p - публичный порт
-s - приватный порт для админа ноды)

5) `http://127.0.0.1:8000/api/v1/config/actual` - посмотреть актуальный конфиг и его хэш. Это важно, чтобы указать в следуещем конфиге ссылку на предыдущий в поле `"previous_cfg_hash": "7aa8c6a39438443ccc2a31d28cbd6b3f897be9db3ff13711b85a3eddb5eebcba"`

`http://127.0.0.1:8000/api/v1/config/following` - посмотреть принятый, но еще не вступивший в силу следующий конфиг, и его хэш.

`http://127.0.0.1:8000/api/v1/configs/7aa8c6a39438443ccc2a31d28cbd6b3f897be9db3ff13711b85a3eddb5eebcba` - посмотреть данные о конфиге по его хэшу

`http://127.0.0.1:8000/api/v1/configs/7aa8c6a39438443ccc2a31d28cbd6b3f897be9db3ff13711b85a3eddb5eebcba/votes` - посмотреть принятые голоса

 6) внести предложение за новый конфиг: 
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

7) вот так вот проголосовать за один и тот же конфиг разными валидаторами по его cfg_hash 
 ```curl -X POST -d '{}' http://127.0.0.1:8010/api/v1/configs/a0803af8d614ab7484ebc3cf605adfb1a600161d905bfa7fb75ca26a543d0e0f/postvote
curl -X POST -d '{}' http://127.0.0.1:8011/api/v1/configs/a0803af8d614ab7484ebc3cf605adfb1a600161d905bfa7fb75ca26a543d0e0f/postvote
curl -X POST -d '{}' http://127.0.0.1:8012/api/v1/configs/a0803af8d614ab7484ebc3cf605adfb1a600161d905bfa7fb75ca26a543d0e0f/postvote
```
