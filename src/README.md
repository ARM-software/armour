# Armour source code

## Armour Components

- `armour-api/`: types used in Armour APIs
- `armour-compose/`: support for serializing and deserializing docker-compose files, extended for use with Armour
- `armour-control/`: Armour control plane (with RESTful interface)
- `armour-ctl/`: command line tool for communicating with `armour-control`
- `amrour-lang/`: implementation of Armour policy language (provides REPL for testing)
- `armour-lauch/`: tool, similar to docker-compose, for starting and stopping Armour secured services
- `armour-master`: data plane master. Manages communication between `armour-control`, `armour-proxy` and `armour-launch`. Provides interactive shell and RESTful interface.
- `armour-proxy`: data plane proxy. Enforces Armour policies.
- `armour-serde`: additional library code for working with [serde](https://serde.rs)
- `armour-utils`: general library (shared code)
- `docker-api`: provides interface to a local docker engine (used by `armour-launch`)

## Other directories

- `docs/`: documentation and API testing
- `experimental/`: developmental code, not yet integrated into the current version of Armour
- `policies/`: policy files (examples and testing)
- `tools/`: utilities for testing Armour
