
# Table of Contents

1.  [Setup a docker-machine](#orge574eeb)
    1.  [Create the machine](#orgae8f0e7)
    2.  [Set up the docker environment to the docker machine](#org66f2cc6)
    3.  [ssh into the machine](#org78e421a)
    4.  [Set up a shared directory with the VM host (docker-machine mount)](#org8534997)
2.  [Dockerfiles](#org1f27521)
    1.  [dockerfile-client-python](#orge2ee4c4)
3.  [docker-compose file](#orgd4ac30e)
    1.  [Rebuild images if necessary](#org19cb8c3)
    2.  [Run the images](#org66d9f73)
    3.  [Set up the iptables rules](#orgdbfdd04)
    4.  [Armour-playground](#orgb50d097)
    5.  [Testing example (03-04-2019 no filtering, only proxying):](#org1f25c4b)
4.  [Setting up Rust for cross-compilation](#orgc751688)
    1.  [Cross Compiling Static Rust Binaries for Linux on OS X · Graham Enos](#orgfa63643)
    2.  [Easy Windows and Linux cross-compilers for macOS](#org0b41f61)
    3.  [Cross-compile and link a static binary on macOS for Linux with cargo and rust - chr4](#org0a1dbef)


<a id="orge574eeb"></a>

# Setup a docker-machine


<a id="orgae8f0e7"></a>

## Create the machine

    docker-machine create armour


<a id="org66f2cc6"></a>

## Set up the docker environment to the docker machine

    eval (docker-machine env armour)


<a id="org78e421a"></a>

## ssh into the machine

    docker-machine ssh armour


<a id="org8534997"></a>

## Set up a shared directory with the VM host [(docker-machine mount)](https://docs.docker.com/machine/reference/mount/)

-   Requires sshfs

    docker-machine ssh armour mkdir armour-playground
    docker-machine mount ./armour-playground armour:/dm-mount-point/

-   This solution is only temporary, to make it permanent add a rule to the VBox machine settings


<a id="org1f27521"></a>

# Dockerfiles


<a id="orge2ee4c4"></a>

## dockerfile-client-python

-   Creates a simple image which can run python servers
-   Exposes port 8080 // This is actually not necessary since it will be
    taken care of by the ip-tables rules
-   mounts the armour-playground directory above (where python files are stored)


<a id="orgd4ac30e"></a>

# docker-compose file


<a id="org19cb8c3"></a>

## Rebuild images if necessary

    docker-compose build
    # docker-compose up -d build


<a id="org66d9f73"></a>

## Run the images

    docker-compose up -d


<a id="orgdbfdd04"></a>

## Set up the iptables rules

    docker-machine ssh armour
    sudo sh /dm-mount-point/iptables-setup.sh
    exit


<a id="orgb50d097"></a>

## Armour-playground

-   All images mount the armour-playground directory where both the
    servers and infrastructure binaries are stored to be updated from
    the host and reduce development setup time


<a id="org1f25c4b"></a>

## Testing example (03-04-2019 no filtering, only proxying):

-   run a server in server-1

    docker exec -it server-1 python3 /armour-playground/flask-server/server.py -d

-   TODO: use DNS to avoid hardcoded IPs
    -   get the ip address of server-1 and server-2

    docker exec -it server-1 ip addr show dev eth0 | grep inet | cut -f1 -d '/'
    docker exec -it server-1 ip addr show dev eth0 | grep inet | cut -f1 -d '/'

-   make a request in server-2

    docker exec -it server-2 curl http://10.4.0.2:8080/

-   Run the proxy in a different terminal

    docker exec -it proxy /armour-playground/arm-proxy/x86_64-unknown-linux-musl/debug/arm-proxy -i eth0

-   Repeat the request

    docker exec -it server-2 curl http://10.4.0.2:8080/

It should fail with a forward error at this point

-   Allow the request:

    docker exec -it proxy curl http://10.3.0.2:8444/allow/10.5.0.2/10.4.0.2/8080

-   Repeat the request

    docker exec -it server-2 curl http://10.4.0.2:8080/

It should succeed!

-   Repeat the process with each of the client/servers needed


<a id="orgc751688"></a>

# Setting up Rust for cross-compilation


<a id="orgfa63643"></a>

## [Cross Compiling Static Rust Binaries for Linux on OS X · Graham Enos](https://grahamenos.com/rust-osx-linux-musl.html)


<a id="org0b41f61"></a>

## [Easy Windows and Linux cross-compilers for macOS](https://blog.filippo.io/easy-windows-and-linux-cross-compilers-for-macos/)


<a id="org0a1dbef"></a>

## [Cross-compile and link a static binary on macOS for Linux with cargo and rust - chr4](https://chr4.org/blog/2017/03/15/cross-compile-and-link-a-static-binary-on-macos-for-linux-with-cargo-and-rust/)

