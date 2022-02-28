# blobstore-fs Makefile

CAPABILITY_ID = "wasmcloud:blobstore_fs"
NAME = "Blobstore for unix file system"
VENDOR = "Wasmcloud"
PROJECT = blobstore_fs
VERSION  = $(shell cargo metadata --no-deps --format-version 1 | jq -r '.packages[] .version' | head -1)
REVISION = 0

# builds are 'release'
# If using debug builds, change bin_path in provider_test_config.toml
TEST_FLAGS := --release -- --nocapture

include ../build/makefiles/provider.mk

ifeq ($(shell nc -zt -w1 127.0.0.1 4222 || echo fail),fail)
test::
	@killall --quiet blobstore_fs
	docker run --rm -d --name fs-provider-test -p 127.0.0.1:4222:4222 nats:2.7.2 -js
	cargo test $(TEST_FLAGS)
	docker stop fs-provider-test
else
test::
	@killall --quiet blobstore_fs
	cargo test $(TEST_FLAGS)
endif

