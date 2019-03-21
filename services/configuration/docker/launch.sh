#!/usr/bin/env bash

node_count=$1
start_peer_port=6331
start_public_port=8000
path_to_app=/root/.cargo/bin/configuration

if (($node_count<=0));
then
echo "Invalid node count value: $node_count. Node count must be > 0."
exit 1
fi

$path_to_app generate-template common.toml --validators-count $node_count

for i in $(seq 0 $((node_count - 1)))
do
  peer_port=$((start_peer_port + i))
  $path_to_app generate-config common.toml pub_$((i + 1)).toml sec_$((i + 1)).toml --peer-address 127.0.0.1:${peer_port} -c consensus_$((i + 1)).toml -s service_$((i + 1)).toml -n
done

for i in $(seq 0 $((node_count - 1)))
do
	pub_configs="$pub_configs pub_$((i + 1)).toml"
done

for i in $(seq 0 $((node_count - 1)))
do
  public_port=$((start_public_port + i))
  private_port=$((public_port + node_count))
  $path_to_app finalize --public-api-address 0.0.0.0:${public_port} --private-api-address 0.0.0.0:${private_port} sec_$((i + 1)).toml node_$((i + 1))_cfg.toml --public-configs $pub_configs
done

for i in $(seq 0 $((node_count - 1)))
do
  public_port=$((start_public_port + i))
  private_port=$((public_port + node_count))
  $path_to_app run --node-config node_$((i + 1))_cfg.toml --db-path db$((i + 1)) --public-api-address 0.0.0.0:${public_port} --consensus-key-pass pass --service-key-pass pass &
  echo "new node with ports: $public_port (public) and $private_port (private)"
  sleep 1
done

echo "$node_count nodes configured and launched"

while :
do
	sleep 300
done
