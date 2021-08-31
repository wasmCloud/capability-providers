# HTTP Client Capability Provider

This capability provider implements the `wasmcloud:httpclient` capability contract using the Rust [reqwest](https://docs.rs/reqwest) library.

This capability provider is multi-threaded and can handle concurrent requests from multiple actors.

Build with `make`. Test with `make test`.

## Link Definition Values
This capability provider does not have any link definition configuration values.

