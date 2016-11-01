#!/bin/bash

destdir=$2
template=$3
startport=$4

node_executable=cryptocurrency
scriptdir=`dirname "$BASH_SOURCE"`
supervisor_conf=${destdir}/etc/supervisord.conf
sock_file=/tmp/

start() {
    mkdir -p ${destdir}/log/supervisor
    mkdir ${destdir}/run
    mkdir ${destdir}/db
    cp -R ${scriptdir}/${template}/etc ${destdir} || exit 1
    $node_executable -c ${destdir}/testnet.conf generate 9 -p ${startport} || exit 1
    cd $destdir
    supervisord || exit 1
    supervisorctl start leveldb:* || exit 1
}

stop() {
    cd ${destdir}
    if [ -e /tmp/supervisor_${template}_cryptocurrency.sock ]
    then
        supervisorctl shutdown || exit 1
    fi 
    rm -rf "./db"
    rm -rf "./etc"
}

case "$1" in
    stop)
        stop
        ;;
    start) 
        start
        ;;
esac