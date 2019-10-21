#!/usr/bin/env bash

# This script copies protobuf specification files from exonum crates and pushes them as a separate new branch to the
# repo that could be used by various Exonum clients. The branch can then be used as a source for a pull request.
#
# This script is intended to be used by Exonum developers in order to update Exonum clients with stable changes in the
# proto files. No need to execute this script for any intermediate changes.

set -eu -o pipefail

# prints a section header
function header() {
    local title=$1
    local rest="========================================================================"
    echo
    echo "===[ ${title} ]${rest:${#title}}"
    echo
}

PROTO_REPO_URI="https://github.com/exonum/exonum-proto.git"
PROTO_REPO_TMP_DIR="/tmp/_proto_repo_tmp"

BRANCH_RPEFIX="update-proto"

PROTO_ROOT_DIR=$(pwd)
MAIN_PROTO_FILES_DIR=${PROTO_ROOT_DIR}/schema/exonum
COMPONENTS_DIR=$(pwd)/../../../components

# Clean temporary dir from the previous iteration if any
rm -fR ${PROTO_REPO_TMP_DIR}

header "CLONING REPO"

# Checkout repo
git clone ${PROTO_REPO_URI} ${PROTO_REPO_TMP_DIR}

header "COPYING PROTO FILES"

# Copy main files
cp -v ${MAIN_PROTO_FILES_DIR}/blockchain.proto ${PROTO_REPO_TMP_DIR}
cp -v ${MAIN_PROTO_FILES_DIR}/consensus.proto ${PROTO_REPO_TMP_DIR}
cp -v ${MAIN_PROTO_FILES_DIR}/runtime.proto ${PROTO_REPO_TMP_DIR}
# BitVec
cp -v ${COMPONENTS_DIR}/proto/src/proto/common.proto ${PROTO_REPO_TMP_DIR}
# Crypto stuff
cp -v ${COMPONENTS_DIR}/crypto/src/proto/schema/types.proto ${PROTO_REPO_TMP_DIR}

header "ADDING PROTO FILES TO THE REPO"

cd ${PROTO_REPO_TMP_DIR}

# Create a new branch for changes and push
BRANCH_NAME=${BRANCH_RPEFIX}-$(date "+%Y.%m.%d-%H%M%S")
git checkout -b ${BRANCH_NAME}
git add ${PROTO_REPO_TMP_DIR}/.
git commit -m "Updating proto files"
git push origin ${BRANCH_NAME}

cd ${PROTO_ROOT_DIR}

header "DONE"
echo "You can now create pull request from the [${BRANCH_NAME}] branch or adjust files in the [${PROTO_REPO_TMP_DIR}] if required."
