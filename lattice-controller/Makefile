# capability-provider/lattice-controller/Makefile

PROJECT = lattice-controller
CAPABILITY_ID = wasmcloud:latticecontrol
VENDOR   = "wasmCloud"
VERSION  = $(shell cargo metadata --no-deps --format-version 1 | jq -r '.packages[] .version' | head -1)
REVISION = 0
oci_url  = localhost:5000/v2/$(PROJECT):$(VERSION)

include ../build/makefiles/provider.mk

test:
	killall target/debug/lattice-controller || true
	RUST_BACKTRACE=1 cargo test -- --nocapture
