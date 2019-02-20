#!/usr/bin/env bash

# Usage:
# example-start.sh <number of nodes>

node_count=$1
port_start=8000
port_end=$((port_start + (2 * node_count) - 1))

docker run -p ${port_start}-${port_end}:${port_start}-${port_end} exonumhub/exonum-configuration-service:example $node_count
