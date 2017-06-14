#!/bin/bash

if [ -z "$SERVICE_ROOT" ]; then
    echo "Need to set SERVICE_ROOT"
    exit 1
fi

destdir=$SERVICE_ROOT
scriptdir="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
supervisor_conf=${destdir}/etc/supervisord.conf

install() {
    echo "Installing to: ${destdir}"
    if [ -d "${destdir}/etc" ]; then
        echo "Already installed here"
        return
    fi

    echo "Build frontend..."
    cd ${scriptdir}/../frontend
    npm install
    cd -

    echo "Build backend..."
    cd ${scriptdir}/../backend
    cargo build -p cryptocurrency
    cd -

    echo "Create supervisor environment..."
    mkdir -p ${destdir}/log/supervisor
    mkdir ${destdir}/run
    mkdir ${destdir}/var
    rsync -rt ${scriptdir}/supervisord/etc/ ${destdir}/etc || exit 1
    ln -s ${scriptdir}/../frontend ${destdir}/frontend
    ln -s ${scriptdir}/../backend ${destdir}/backend

    echo "Generate new configuration for nodes..."
    ${destdir}/backend/target/debug/cryptocurrency generate -o ${destdir}/etc 6 -p 2000
    validators=$(cat ${destdir}/etc/validators/0.toml | sed -n -e 's/validators = //p')
    echo "Use validators: $validators"
    cat ${destdir}/frontend/config-example.json | sed -r "s/(\"validators\": )(\[\])/\1${validators}/" > ${destdir}/frontend/config.json
}

enable() {
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

    svcgroup=$1
    cd ${destdir}
    supervisorctl update ${svcgroup}
    supervisorctl start ${svcgroup}:* || exit 1
}

restart() {
    test -e /tmp/supervisord.sock || exit 1

    svcgroup=$1
    cd ${destdir}
    supervisorctl restart ${svcgroup}:* || exit 1
}

stop() {
    test -e /tmp/supervisord.sock || exit 1

    svcgroup=$1
    cd ${destdir}
    supervisorctl stop ${svcgroup}:* || exit 1
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
    install)
        install $2
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
    *)
        echo "Exonum cryptocurrency demo bootstrap script"
        echo "Usage: ./bootstrap.sh [start|stop|restart|install|enable|disable|update|clear|generate]"
        ;;
esac
