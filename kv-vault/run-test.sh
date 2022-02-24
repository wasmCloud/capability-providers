#!/usr/bin/env bash

# This script starts vault in dev mode in a docker container, and runs cargo tests
set -e

# name of host and port. Port should be unique to avoid conflicting with other open ports
LOCAL_IP=127.0.0.1:11182
# name of vault's temporary docker container
CONTAINER_NAME=kv-vault-test
# mount point, default is "secret"
VAULT_MOUNT=secret
# debug setting for rust test code
RUST_LOG=debug
# build release flag, empty for debug, or '--release' for release build
# If changed, make sure it's consistent with 'bin_path' in provider_test_config.toml
RELEASE_FLAG=--release

cleanup() {
    docker rm -f ${CONTAINER_NAME} 2>/dev/null
    killall -q kv-vault || true
}

# make sure it's built
cargo build ${RELEASE_FLAG} --all-features --all-targets
cleanup

# start vault docker in dev mode, no tls
docker run --rm -d \
  --cap-add=IPC_LOCK \
  --name ${CONTAINER_NAME} \
  -p ${LOCAL_IP}:8200 \
  vault:latest
sleep 2
export VAULT_TOKEN="$(docker logs ${CONTAINER_NAME} 2>&1 | grep 'Root Token:' | sed -E 's/Root Token: //')"
# secret/ is mounted automatically so --path arg should only be used if VAULT_MOUNT is something else
[[ -n "$VAULT_MOUNT" ]] && [[ "$VAULT_MOUNT" != "secret" ]] && [[ "$VAULT_MOUNT" != "secret/" ]] && PATH_ARG=-path=$VAULT_MOUNT
docker exec -i -e VAULT_TOKEN=${VAULT_TOKEN} ${CONTAINER_NAME} \
    vault secrets enable -version=2 -local \
        -address=http://127.0.0.1:8200 $PATH_ARG kv

# run cargo test
export RUST_BACKTRACE=1
export RUST_LOG=${RUST_LOG}
export VAULT_ADDR=http://${LOCAL_IP}
[ -n "$VAULT_MOUNT" ] && export VAULT_MOUNT=${VAULT_MOUNT}
cargo test ${RELEASE_FLAG} -- --nocapture

# cleanup
cleanup

