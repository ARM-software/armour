
# Table of Contents

1.  [Setup a docker-machine](#org329ad33)
    1.  [Create the machine](#orgd370b32)
    2.  [Set up the docker environment to the docker machine](#org5287b60)
    3.  [ssh into the machine](#orgf7c75db)
    4.  [Set up a shared directory with the VM host (docker-machine mount)](#orgdee5337)
2.  [Dockerfiles](#orgcda400d)
    1.  [dockerfile-client-python](#org32b82cd)
3.  [docker-compose file](#org1d36a18)
    1.  [Rebuild images if necessary](#org6ced9b4)
    2.  [Run the images](#orgd7f975e)
    3.  [Set up the iptables rules](#org4a63991)
    4.  [Armour-playground](#org4cfbe14)
    5.  [Testing example (03-04-2019 no filtering, only proxying):](#org965d3b1)
4.  [Setting up Rust for cross-compilation](#org210a187)
    1.  [Cross Compiling Static Rust Binaries for Linux on OS X · Graham Enos](#org2265ebf)
    2.  [Easy Windows and Linux cross-compilers for macOS](#org916b619)
    3.  [Cross-compile and link a static binary on macOS for Linux with cargo and rust - chr4](#org6296983)


<a id="org329ad33"></a>

# Setup a docker-machine


<a id="orgd370b32"></a>

## Create the machine

    docker-machine create armour


<a id="org5287b60"></a>

## Set up the docker environment to the docker machine

    eval (docker-machine env armour)


<a id="orgf7c75db"></a>

## ssh into the machine

    docker-machine ssh armour


<a id="orgdee5337"></a>

## Set up a shared directory with the VM host [(docker-machine mount)](https://docs.docker.com/machine/reference/mount/)

-   Requires sshfs

    docker-machine ssh armour mkdir armour-playground
    docker-machine mount ./armour-playground armour:/dm-mount-point/

-   This solution is only temporary, to make it permanent add a rule to the VBox machine settings


<a id="orgcda400d"></a>

# Dockerfiles


<a id="org32b82cd"></a>

## dockerfile-client-python

-   Creates a simple image which can run python servers
-   Exposes port 8080 // This is actually not necessary since it will be
    taken care of by the ip-tables rules
-   mounts the armour-playground directory above (where python files are stored)


<a id="org1d36a18"></a>

# docker-compose file


<a id="org6ced9b4"></a>

## Rebuild images if necessary

    docker-compose build
    # docker-compose up -d build


<a id="orgd7f975e"></a>

## Run the images

    docker-compose up -d


<a id="org4a63991"></a>

## Set up the iptables rules

    docker-machine ssh armour
    sudo sh /dm-mount-point/iptables-setup.sh
    exit


<a id="org4cfbe14"></a>

## Armour-playground

-   All images mount the armour-playground directory where both the
    servers and infrastructure binaries are stored to be updated from
    the host and reduce development setup time


<a id="org965d3b1"></a>

## Testing example (03-04-2019 no filtering, only proxying):

-   Cargo target should be in to $SFPL/sfpl2-drafts/armour-playground/cargo-target/
-   run a server in server-1

    docker exec -it server-1 python3 /flask-server/server.py -d

-   TODO: use DNS to avoid hardcoded IPs
    -   get the ip address of server-1 and server-2

    docker exec -it server-1 ip addr show dev eth0 | grep inet | cut -f1 -d '/'
    docker exec -it server-1 ip addr show dev eth0 | grep inet | cut -f1 -d '/'

-   make a request in server-2

    docker exec -it server-2 curl http://10.4.0.2:8080/

-   Run the proxy in a different terminal

    docker exec -it proxy /armour-playground/cargo-target/x86_64-unknown-linux-musl/debug/arm-proxy -i eth0

-   Repeat the request

    docker exec -it server-2 curl http://10.4.0.2:8080/

It should fail with a forward error at this point

-   Allow the request:

    docker exec -it proxy curl http://10.3.0.2:8444/allow/10.5.0.2/10.4.0.2/8080

-   Repeat the request

    docker exec -it server-2 curl http://10.4.0.2:8080/

It should succeed!

-   Repeat the process with each of the client/servers needed


<a id="org210a187"></a>

# Setting up Rust for cross-compilation


<a id="org2265ebf"></a>

## [Cross Compiling Static Rust Binaries for Linux on OS X · Graham Enos](https://grahamenos.com/rust-osx-linux-musl.html)


<a id="org916b619"></a>

## [Easy Windows and Linux cross-compilers for macOS](https://blog.filippo.io/easy-windows-and-linux-cross-compilers-for-macos/)


<a id="org6296983"></a>

## [Cross-compile and link a static binary on macOS for Linux with cargo and rust - chr4](https://chr4.org/blog/2017/03/15/cross-compile-and-link-a-static-binary-on-macos-for-linux-with-cargo-and-rust/)

