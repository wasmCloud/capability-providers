# capability-provider/kv-dynamodb/Makefile

PROJECT = kv_dynamodb
CAPABILITY_ID = wasmcloud:keyvalue
VENDOR   = "wasmCloud"
NAME     = "DynamoDB KeyValue Store"
VERSION  = $(shell cargo metadata --no-deps --format-version 1 | jq -r '.packages[] .version' | head -1)
REVISION = 0
oci_url  = localhost:5000/v2/$(PROJECT):$(VERSION)

include ../build/makefiles/provider.mk

test:
	killall target/debug/kv_dynamodb || true
	RUST_BACKTRACE=1 cargo test -- --nocapture
