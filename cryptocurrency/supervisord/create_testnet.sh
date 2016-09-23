#!/bin/bash

node_executable=cryptocurrency
destdir="/tmp/exonum"
scriptdir=`dirname "$BASH_SOURCE"`
supervisor_conf=${destdir}/etc/supervisord.conf

mkdir -p ${destdir}/log/supervisor
mkdir ${destdir}/run
mkdir ${destdir}/db
cp -R ${scriptdir}/etc ${destdir}
$node_executable -c /tmp/exonum/testnet.conf generate 4

echo "--> To start testnet:"
echo "supervisord -c ${supervisor_conf}"
echo "--> And after it just run:"
echo "supervisorctl -c ${supervisor_conf}"
echo "Testnet directory is situated in ${destdir}"
