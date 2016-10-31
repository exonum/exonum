#!/bin/bash

node_executable=cryptocurrency
destdir=$1
scriptdir=`dirname "$BASH_SOURCE"`
supervisor_conf=${destdir}/etc/supervisord.conf

mkdir -p ${destdir}/log/supervisor
mkdir ${destdir}/run
mkdir ${destdir}/db
cp -R ${scriptdir}/etc ${destdir}
$node_executable -c ${destdir}/testnet.conf generate 9 -p 9000

echo "--> To start testnet:"
echo "cd ${destdir}"
echo "supervisord"
echo "--> And after it just run:"
echo "supervisorctl"
echo "Testnet directory is situated in ${destdir}"
