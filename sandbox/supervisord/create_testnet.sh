#!/bin/bash

scriptdir=`dirname "$BASH_SOURCE"`
supervisor_conf=${scriptdir}/etc/supervisord.conf

mkdir -p /tmp/exonum/log/supervisor
mkdir /tmp/exonum/run
mkdir /tmp/exonum/db
test_node -c /tmp/exonum/testnet.conf generate 16

echo "--> To start testnet:"
echo "supervisord -c ${supervisor_conf}"
echo "--> And after it just run:"
echo "supervisorctl -c ${supervisor_conf}"
echo "Testnet directory is situated in /tmp/exonum"
