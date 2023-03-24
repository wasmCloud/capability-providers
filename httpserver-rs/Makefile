# capability-provider/httpserver-rs/Makefile

# short name that will be used for the executable name
PROJECT  = httpserver
CAPABILITY_ID = wasmcloud:httpserver
VENDOR   = "wasmCloud"
NAME     = "HTTP Server"
VERSION  = $(shell cargo metadata --no-deps --format-version 1 | jq -r '.packages[] .version' | head -1)
# During development iterations, REVISION should be incremented each build
REVISION = 0
oci_url  = localhost:5000/v2/$(PROJECT):$(VERSION)
oci_insecure = --insecure

include ../build/makefiles/provider.mk

ifeq ($(shell nc -zt -w1 127.0.0.1 4222 || echo fail),fail)
test::
	@echo To run these tests, you need a wamcloud host and nats server running
	exit 2
else
test::
	cargo test -- --nocapture
endif

