#!/usr/bin/env bash

node_count=4
start_peer_port=6331
start_public_port=8200
start_private_port=8091

cd backend && mkdir example && cd example
/root/.cargo/bin/exonum-cryptocurrency-advanced generate-template common.toml --validators-count 4

for i in $(seq 0 $((node_count - 1)))
do
  peer_port=$((start_peer_port + i))
  /root/.cargo/bin/exonum-cryptocurrency-advanced generate-config common.toml pub_$((i + 1)).toml sec_$((i + 1)).toml --peer-address 127.0.0.1:${peer_port}
done

for i in $(seq 0 $((node_count - 1)))
do
  public_port=$((start_public_port + i))
  private_port=$((start_private_port + i))
  /root/.cargo/bin/exonum-cryptocurrency-advanced finalize --public-api-address 0.0.0.0:${public_port} --private-api-address 0.0.0.0:${private_port} sec_$((i + 1)).toml node_$((i + 1))_cfg.toml --public-configs pub_1.toml pub_2.toml pub_3.toml pub_4.toml
done

for i in $(seq 0 $((node_count - 1)))
do
  public_port=$((start_public_port + i))
  /root/.cargo/bin/exonum-cryptocurrency-advanced run --node-config node_$((i + 1))_cfg.toml --db-path db$((i + 1)) --public-api-address 0.0.0.0:${public_port} &
  echo "new node with ports: $public_port (public)"
done

cd ../../frontend
npm install
npm run build
npm start -- --port=8280 --api-root=http://127.0.0.1:${start_public_port}
