#!/bin/bash

if [ -z "$TESTNET_DESTDIR" ]; then
    echo "Need to set TESTNET_DESTDIR"
    exit 1
fi 

destdir=$TESTNET_DESTDIR
scriptdir=`dirname "$BASH_SOURCE"`
supervisor_conf=${destdir}/etc/supervisord.conf

enable() {
    mkdir -p ${destdir}/log/supervisor
    mkdir ${destdir}/run
    rsync -rt ${scriptdir}/supervisord/etc/ ${destdir}/etc || exit 1

    cd ${destdir}
    if [ -e /tmp/supervisord.sock ]
    then
        supervisorctl reload || exit 1
    else
        supervisord || exit 1
    fi 
}

disable() {
    cd ${destdir}
    if [ -e /tmp/supervisord.sock ]
    then
        supervisorctl shutdown || exit 1
    fi 
}

update() {
    test -e /tmp/supervisord.sock || exit 1

    rsync -rt ${scriptdir}/supervisord/etc/ ${destdir}/etc/ || exit 1
    cd ${destdir}
    supervisorctl reread || exit 1
}

clear() {
    rm -rf ${destdir}/${1}
}

start() {
    test -e /tmp/supervisord.sock || exit 1

    template=$1
    cd ${destdir}
    supervisorctl update ${template}
    supervisorctl start ${template}:* || exit 1
}

restart() {
    test -e /tmp/supervisord.sock || exit 1

    template=$1
    cd ${destdir}
    supervisorctl restart ${template}:* || exit 1
}

stop() {
    test -e /tmp/supervisord.sock || exit 1

    template=$1
    cd ${destdir}
    supervisorctl stop ${template}:* || exit 1
}

generate() {
    template=$1
    count=$2
    port=$3

    test -e ${destdir}/${template} && exit 1
    exonumctl generate -o ${destdir}/${template} $count -p $port
}

case "$1" in
    start)
        start $2
        ;;
    stop) 
        stop $2
        ;;
    restart)
        restart $2
        ;;
    enable) 
        enable $2
        ;;
    disable) 
        disable $2
        ;;
    update) 
        update $2
        ;;
    clear) 
        clear $2
        ;;
    generate) 
        generate $2 $3 $4
        ;;
esac