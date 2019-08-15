Structure
=========

## Rust code

- `armour/`: implementation of Armour (language and data plane)
- `externals/`: support for developing Armour "policy services" (aka oracles) in rust
- `testing/`: utilities for testing Armour
- `experimental/`: developmental code not yet integrated into the current version of Armour

## Build support

- `docker/`: scripts for compiling Armour code and for building docker containers that contain the output binaries
- `vagrant/`: support for building Armour Linux binaries in a Vagrant VM
- `musl/`: similar to `vagrant/` except that it uses [cross](https://github.com/rust-embedded/cross), which could be used to produce Arm binaries

Prerequisites
=============

- [Rust](https://www.rust-lang.org/tools/install)

    ```shell
    $ curl https://sh.rustup.rs -sSf | sh
    ```
    
- [Cap'n Proto](https://capnproto.org/install.html)

    - Mac
    
        ```shell
        $ brew install capnp
        ```
    - Linux

        ```shell
        $ apt-get install capnproto
        ```

- OpenSSL

    - Mac

        ```shell
        $ brew install openssl
        ```

    - Linux

        ```shell
        $ apt-get install openssl libssl-dev
        ```
