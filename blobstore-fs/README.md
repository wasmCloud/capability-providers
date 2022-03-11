# blobstore-fs capability provider

This capability provider implements the "wasmcloud:blobstore" capability for
Unix file system. The provider will store files in the local host where the
provider executes.

Currently file upload (put_object) and download (get_object) are not chunked 
and sends/retrieves the entire file in one go. 

## Building

Build with 'make'. Test with 'make test'.
Testing requires docker.

## Configuration

The provider is configured with `ROOT=<path>` which specifies where files will be stored/read.
The default root path is `/tmp`. The provider must have read and write access to the root location.
Each actor will store its files under the directory `$ROOT/<actor_id>`.

