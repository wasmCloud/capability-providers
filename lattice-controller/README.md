# Lattice Controller Capability Provider

A capability provider that allows actors to interact with the lattice control interface (`wasmcloud:latticecontrol`) by
remotely communicating with lattices and the hosts contained within them via NATS.

## Configuration
This capability provider is designed to facilitate connections to multiple lattices. For each connection that actors need to utilize, there must be an accompanying `set_lattice_credentials` call, _even if you are establishing a connection to the `default` lattice_.

It may not be immediately obvious, but because of the flexible design of wasmCloud's lattice, the actor(s) that set credentials need not be the same actor(s) that utilize the established connections so long as they are all bound to the provider via empty link definitions.

Further, you can run multiple instances of this provider in a source lattice (e.g. not necessarily one you're remotely managing) and it will automatically scale, with each instance maintaining its own cached connection to the appropriate lattices.

## ⚠️ Compatibility Warning for versions < 0.9.0 ⚠️
In previous versions of this capability provider, the provider would only ever establish one lattice control connection per instance, typically associated with the link name. The configuration for the lattice connection would come from the data on the link definition.

This is _not_ how the current version of the provider works. The current provider is a _multiplexed_ provider supporting multiple lattices. An actor needs to establish a link once via an empty link definition, and then establish connections to remote lattices by using the `set_lattice_credentials` operation on the provider.

This provider no longer supports fallback connections supplied via the provider configuration parameter at startup. In other words, you _must_ invoke `set_lattice_credentials` at least once to use this provider.



