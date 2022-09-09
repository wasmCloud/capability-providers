# Capability Providers

This repository contains capability providers for wasmCloud. The providers 
in the root level of this repository are _only_ compatible with version `0.50`
and _newer_ of wasmCloud. All of the pre-existing capability providers compatible
with `0.18` (aka "pre-OTP") or earlier can be found in the [pre-otp](./pre-otp) folder.

## First-Party Capability Providers
The following is a list of first-party supported capability providers developed by the
wasmCloud team.

| Provider | Contract | Description | OCI Reference <img style="width: 300px" align="right" />  |
| :--- | :--- | :--- | :--- |
| [blobstore-fs](./blobstore-fs) | [`wasmcloud:blobstore`](https://github.com/wasmCloud/interfaces/tree/main/blobstore-fs) | Blobstore implementation where blobs are local files and containers are folders | <img alt='blobstore fs oci reference' src='https://img.shields.io/endpoint?url=https%3A%2F%2Fproud-bird-4896.cosmonic.io%2Fblobstore-fs' />
| [blobstore-s3](./blobstore-s3) | [`wasmcloud:blobstore`](https://github.com/wasmCloud/interfaces/tree/main/blobstore-s3) | Blobstore implementation with AWS S3 | <img alt='blobstore s3 oci reference' src='https://img.shields.io/endpoint?url=https%3A%2F%2Fproud-bird-4896.cosmonic.io%2Fblobstore-s3' />
| [httpserver](./httpserver-rs) | [`wasmcloud:httpserver`](https://github.com/wasmCloud/interfaces/tree/main/httpserver) | HTTP web server built with Rust and warp/hyper | <img alt='httpserver oci reference' src='https://img.shields.io/endpoint?url=https%3A%2F%2Fproud-bird-4896.cosmonic.io%2Fhttpserver' />
| [httpclient](./httpclient) | [`wasmcloud:httpclient`](https://github.com/wasmCloud/interfaces/tree/main/httpclient) | HTTP client built in Rust |  <img alt='httpclient oci reference' src='https://img.shields.io/endpoint?url=https%3A%2F%2Fproud-bird-4896.cosmonic.io%2Fhttpclient' />
| [redis](./kvredis) | [`wasmcloud:keyvalue`](https://github.com/wasmCloud/interfaces/tree/main/keyvalue) | Redis-backed key-value implementation | <img alt='kvredis oci reference' src='https://img.shields.io/endpoint?url=https%3A%2F%2Fproud-bird-4896.cosmonic.io%2Fkvredis' />
| [vault](./kv-vault) | [`wasmcloud:keyvalue`](https://github.com/wasmCloud/interfaces/tree/main/keyvalue) | Vault-backed key-value implementation for secrets | <img alt='kv-vault oci reference' src='https://img.shields.io/endpoint?url=https%3A%2F%2Fproud-bird-4896.cosmonic.io%2Fkv-vault' />
| [nats](./nats) | [`wasmcloud:messaging`](https://github.com/wasmCloud/interfaces/tree/main/messaging) | [NATS](https://nats.io)-based message broker | <img alt='nats oci reference' src='https://img.shields.io/endpoint?url=https%3A%2F%2Fproud-bird-4896.cosmonic.io%2Fnats_messaging' />
| [lattice-controller](./lattice-controller) | [`wasmcloud:latticecontroller`](https://github.com/wasmCloud/interfaces/tree/main/lattice-controller) | Lattice Controller interface | <img alt='lattice-controller oci reference' src='https://img.shields.io/endpoint?url=https%3A%2F%2Fproud-bird-4896.cosmonic.io%2Flattice-controller' />
| [postgres](./sqldb-postgres) | [`wasmcloud:sqldb`](https://github.com/wasmCloud/interfaces/tree/main/sqldb) | Postgres-based SQL database capability provider | <img alt='sqldb-postgres oci reference' src='https://img.shields.io/endpoint?url=https%3A%2F%2Fproud-bird-4896.cosmonic.io%2Fsqldb-postgres' />

## Built-in Capability Providers
The following capability providers are included automatically in every host runtime:

| Provider | Contract | Description |
| :--- | :--- | :--- |
| **N/A** | [`wasmcloud:builtin:numbergen`](https://github.com/wasmCloud/interfaces/tree/main/numbergen) | Number generator, including random numbers and GUID strings |
| **N/A** | [`wasmcloud:builtin:logging`](https://github.com/wasmCloud/interfaces/tree/main/logging) | Basic level-categorized text logging capability |

While neither of these providers requires a _link definition_, to use either of them your actors _must_ be signed with their contract IDs.

## Additional Examples
Additional capability provider examples and sample code can be found in the [wasmCloud examples](https://github.com/wasmCloud/examples) repository.