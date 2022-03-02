# Makefile for blobview

PROJECT  = blobview
VERSION  = $(shell cargo metadata --no-deps --format-version 1 | jq -r '.packages[] .version' | head -1)
REVISION = 0
# list of all contract claims for actor signing (space-separated)
CLAIMS   = wasmcloud:httpserver wasmcloud:blobstore wasmcloud:builtin:logging
# registry url for our actor
REG_URL  = localhost:5000/v2/$(PROJECT):$(VERSION)
# command to upload to registry (without last wasm parameter)
PUSH_REG_CMD = wash reg push --insecure $(REG_URL)

# friendly name for the actor
ACTOR_NAME = "blobview"
# optional call alias for actor
ACTOR_ALIAS=blobview

include ./actor.mk

WASH=wash
HTTPSERVER_REF=wasmcloud.azurecr.io/httpserver:0.14.10
HTTPSERVER_ID=VAG3QITQQ2ODAOWB5TTQSDJ53XK3SHBEIFNK4AYJ5RKAX2UNSCAPHA5M
ACTOR_ID=$(shell make actor_id)

BLOBSTORE_REF=localhost:5000/v2/blobstore-s3:0.1.0
BLOBSTORE_ID=VC4FX45M2L5G4DN7FZUBMMVM57WKMVIM4EQSN5UV4PNGZ3ETKLFVGP3F

start-providers:
	$(WASH) ctl start provider $(HTTPSERVER_REF)
	$(WASH) ctl start provider $(BLOBSTORE_REF)

link-providers:
	$(WASH) ctl link put $(ACTOR_ID) $(HTTPSERVER_ID) wasmcloud:httpserver PORT=4000
	$(WASH) ctl link put $(ACTOR_ID) $(BLOBSTORE_ID) wasmcloud:blobstore


ACTOR_VERSION=$(shell cargo metadata --format-version=1  | jq -r '.packages[17].version')

update:
	$(MAKE)
	$(MAKE) push
	wash ctl update actor \
		NAUOHU25PMHVRPCSSQ3SAGQH26D63R6QEYMI2O2UF7NLQWXFSVJI7KST \
		MCWAGSP5VP3PJCZ47JYJSSTTUYR5ZJ236RHJD7PGMSNKVF7XG4527NCW \
		localhost:5000/v2/blobview:$(ACTOR_VERSION)

test::
	cargo clippy --all-features --all-targets
