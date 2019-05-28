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
- Musl on Mac

    ```shell
    $ brew install musl-cross
    ```

- OpenSSL

    - Mac

        ```shell
        $ brew install openssl
        ```

    - Linux

        ```shell
        $ apt-get install openssl
        $ apt-get install libssl-dev    
        ```
