# wasmCloud HTTP Server Provider

This executable is an implementation of the `wasmcloud:httpserver` capability. Only actors signed with this capability contract ID will be allowed to use it. 

For more information on the operations supported by this provider, please check out its corresponding [interface](https://github.com/wasmCloud/interfaces/blob/main/httpserver/httpserver.smithy).

Run `make` to compile to a native executable and build the par file.
The par file is created in `build/httpserver.par.gz`.

## Link Definition Configuration Settings
Configuration settings for the httpserver provider are described in [settings](./settings.md). 

The default listen address is 127.0.0.1 port 8000.

### ⚠️ Caution - Port Ownership
If the instance of this capability provider running on a single host is linked to multiple actors attempting to claim the same port, only the first **link definition** for that port will succeed, and the subsequent attempts will fail. During development, 
it is recommended to check ("tail") the wasmCloud host logs for success and error messages.

For more hands-on tutorials on building actors, including HTTP server actors,
see the [wasmcloud.dev](https://wasmcloud.dev) website.
