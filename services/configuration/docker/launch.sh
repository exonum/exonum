#!/usr/bin/env bash

path_to_app=/root/.cargo/bin/configuration

node_count=$1

$path_to_app generate-testnet --start 5400 $node_count --output-dir .

for i in $(seq 0 $((node_count - 1)))
do
	port=$((8000 + i))
	private_port=$((port + node_count))
	$path_to_app run --node-config validators/$i.toml --db-path db/$i --public-api-address 0.0.0.0:${port} --private-api-address 0.0.0.0:${private_port} &
	echo "new node with ports: $port (public) and $private_port (private)"
done

echo "$node_count nodes configured and launched"

while :
do
	sleep 300
done
