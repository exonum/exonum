#!/usr/bin/env bash

node_count=4
start_peer_port=6331
start_public_port=8000
path_to_app=/root/.cargo/bin/exonum-cryptocurrency-advanced

cd backend && mkdir example && cd example
$path_to_app generate-template common.toml --validators-count $node_count

for i in $(seq 0 $((node_count - 1)))
do
  peer_port=$((start_peer_port + i))
  $path_to_app generate-config common.toml $((i + 1)) --peer-address 127.0.0.1:${peer_port} -n
done

for i in $(seq 0 $((node_count - 1)))
do
  public_port=$((start_public_port + i))
  private_port=$((public_port + node_count))
  $path_to_app finalize --public-api-address 0.0.0.0:${public_port} --private-api-address 0.0.0.0:${private_port} $((i + 1))/sec.toml $((i + 1))/node.toml --public-configs {1,2,3,4}/pub.toml
done

for i in $(seq 0 $((node_count - 1)))
do
  public_port=$((start_public_port + i))
  private_port=$((public_port + node_count))
  $path_to_app run --node-config $((i + 1))/node.toml --db-path $((i + 1))/db --public-api-address 0.0.0.0:${public_port} --master-key-pass pass &
  echo "new node with ports: $public_port (public) and $private_port (private)"
  sleep 1
done

echo "Deploying of cryptocurrency-advanced service is in progress..."
sleep 7
python3 -m exonum_launcher -i ../../cryptocurrency_advanced.yaml

# TODO ECR-3882; temporary loop until frontend is disabled
while true; do
    sleep 300
done

# TODO ECR-3882; skip frontend part until JS light client is fixed
#cd ../../frontend
#npm start -- --port=$((start_public_port + 2 * node_count)) --api-root=http://127.0.0.1:${start_public_port}
