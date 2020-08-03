## Armour Sources

Armour has been built using [Rust](https://www.rust-lang.org) version 1.45.1, which can be installed as follows:

```sh
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Dependencies

To build Armour, you will need to install [Cap'n Proto](https://capnproto.org) and [OpenSSL](https://www.openssl.org):

```sh
$ brew install capnp
$ brew install openssl
```

The Armour Control Plane makes use of [MongoDB](https://www.mongodb.com), which can be installed and run on macOS as follows:

```sh
$ brew tap mongodb/brew
$ brew install mongodb-community
$ brew services start mongodb-community
```

> More detailed instructions are available [here](https://docs.mongodb.com/manual/tutorial/install-mongodb-on-os-x).

#### Building Armour

Armour can be built as follows:

```sh
$ cd armour/src
$ cargo build
```

An optimised *release* version of Armour can be built with:

```sh
$ cargo build --release
```
> Compilation is slower when building *release* binaries.

#### Creating Armour Certificates

The tool `armour-certs` can be used to create certificates (used in mTLS connections). The following command creates a set of certificates:

```sh
$ ARMOUR_PASS=armour cargo run -p armour-certs
```

#### Running Armour

Armour provides four main entry-points (see **Armour Components** below):

1. **`armour-control`**
1. **`armour-ctl`**
1. **`armour-host`**
1. **`armour-launch`**
1. **`armour-certs`**

Help pages for these commands can be obtained as follows:

```sh
$ cargo run -p armour-control -- --help
$ cargo run -p armour-ctl -- --help
$ cargo run -p armour-host -- --help
$ cargo run -p armour-launch -- --help
$ cargo run -p armour-certs -- --help
```

The **`armour-host`** component expects a password, which is used to encrypt proxy-to-proxy meta-data. This password can be set using the `ARMOUR_PASS` environment variable, e.g.

```sh
$ ARMOUR_PASS=??? cargo run -p armour-launch
```
> where `???` is the required password.

The **`armour-control`** and **`armour-host`** components provide a RESTful API and the default URLs are:

| component | url |
---|---
| **`armour-control`** | `http://localhost:8088` |
| **`armour-host`** | `http://localhost:8090` |
| **`logger`** (web interface) | `http://localhost:9000` |


## Armour Components

The Armour source code is split into the following components:

- **`armour-api`** : types used in Armour APIs
- **`armour-certs`** : tool for generating certificates (use for mTLS)
- **`armour-compose`** : support for serializing and deserializing docker-compose files, extended for use with Armour
- **`armour-control`** : Armour control plane (with RESTful interface)
- **`armour-ctl`** : command line tool for communicating with `armour-control`
- **`amrour-lang`** : implementation of Armour policy language (provides REPL for experimentation and testing)
- **`armour-lauch`** : tool, similar to docker-compose, for starting and stopping Armour secured services
- **`armour-host`** : data plane host. Manages communication between `armour-control`, `armour-proxy` and `armour-launch`. Provides interactive shell and RESTful interface.
- **`armour-proxy`** : data plane proxy. Enforces Armour policies.
- **`armour-serde`** : additional library code for working with [serde](https://serde.rs)
- **`armour-utils`** : general library (shared code)
- **`docker-api`** : provides interface to a local docker engine (used by `armour-launch`)

## Other directories

- **`docs/`** : control and data plane API testing
- **`experimental/`** : developmental code, not yet integrated into the current version of Armour
- **`policies/`** : policy files (examples and testing)
- **`tools/`** : various Armour related utilities
	- **`arm-service`** : provides simple *HTTP server* and *client* that can be used to test Armour.
	- **`logger`** : Armour oracle that can be used to monitor (log) HTTP and TCP traffic.
	- **`policy-service`** : support for implementing oracles in Rust. Used by `logger`.
	- **`dot-rust`** : fork of [`dot-rust`](https://github.com/przygienda/dot-rust). Used by `logger` to display connectivity graphs.
	- **`hyper-server`** : simple, high performance *HTTP server*, used as a baseline in Armour micro-benchmarking.
