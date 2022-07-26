# sqldb-dynamodb capability provider

This capability provider allows wasmCloud actors to use AWS DynamoDB
through PartiQL, and implements the "wasmcloud:sqldb" capability contract.

Build with 'make'. Test with 'make test'.

### Capabilities:
- execute Parti statements (select, insert, update, etc.)
- query and execute only differ in that query will return the result

### Using the included Github Actions
If you store your source code on Github, we've gone ahead and included two actions: `build.yml` and `release.yml` under `.github/workflows`. The build action will automatically build, lint, and check formatting for your actor. The release action will automatically release a new version of your actor whenever code is pushed to `main`, or when you push a tag with the form `vX.Y.Z`.

These actions require 3 secrets
1. `WASH_ISSUER_KEY`, which can be generated with `wash keys gen issuer`, then look for the 58 character `Seed` value
1. `WASH_SUBJECT_KEY`, which can be generated with `wash keys gen module`, then look for the 58 character `Seed` value
1. `WASMCLOUD_PAT`, which can be created by following the [Github PAT instructions](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token) and ensuring the `write:packages` permission is enabled
