Setup a docker-machine
======================

Create the machine
------------------

``` {.bash}
docker-machine create armour
```

Set up the docker environment to the docker machine
---------------------------------------------------

``` {.bash}
eval (docker-machine env armour)
```

ssh into the machine
--------------------

``` {.bash}
docker-machine ssh armour
```

Set up a shared directory with the VM host [(docker-machine mount)](https://docs.docker.com/machine/reference/mount/)
---------------------------------------------------------------------------------------------------------------------

-   Requires sshfs

``` {.bash}
docker-machine ssh armour mkdir armour-playground
docker-machine mount ./armour-playground armour:/dm-mount-point/
```

-   This solution is only temporary, to make it permanent add a rule to
    the VBox machine settings

Dockerfiles
===========

dockerfile-client-python
------------------------

-   Creates a simple image which can run python servers
-   Exposes port 8080 // This is actually not necessary since it will be
    taken care of by the ip-tables rules
-   mounts the armour-playground directory above (where python files are
    stored)

docker-compose file
===================

Rebuild images if necessary
---------------------------

``` {.bash}
docker-compose build
# docker-compose up -d build
```

Run the images
--------------

``` {.bash}
docker-compose up -d
```

Set up the iptables rules
-------------------------

``` {.bash}
docker-machine ssh armour
sudo sh /dm-mount-point/iptables-setup.sh
exit
```

Armour-playground
-----------------

-   All images mount the armour-playground directory where both the
    servers and infrastructure binaries are stored to be updated from
    the host and reduce development setup time

Testing example (03-04-2019 no filtering, only proxying):
---------------------------------------------------------

-   Cargo target should be in to
    \$SFPL/sfpl2-drafts/armour-playground/cargo-target/
-   run a server in server-1:

``` {.bash}
docker exec -it server-1 python3 /flask-server/server.py -d
```

-- TODO: use DNS to avoid hardcoded IPs

-   get the ip address of server-1 and server-2

``` {.bash}
docker exec -it server-1 ip addr show dev eth0 | grep inet | cut -f1 -d '/'
docker exec -it server-1 ip addr show dev eth0 | grep inet | cut -f1 -d '/'
```

-   make a request in server-2

``` {.bash}
docker exec -it server-2 curl http://10.4.0.2:8080/
```

-   Run the proxy in a different terminal

``` {.bash}
docker exec -it proxy /armour-playground/cargo-target/x86_64-unknown-linux-musl/debug/arm-proxy -i eth0
```

-   Repeat the request

``` {.bash}
docker exec -it server-2 curl http://10.4.0.2:8080/
```

It should fail with a forward error at this point

-   Allow the request:

``` {.bash}
docker exec -it proxy curl http://10.3.0.2:8444/allow/10.5.0.2/10.4.0.2/8080
```

-   Repeat the request

``` {.bash}
docker exec -it server-2 curl http://10.4.0.2:8080/
```

It should succeed!

-   Repeat the process with each of the client/servers needed

Setting up Rust for cross-compilation
=====================================

[Cross Compiling Static Rust Binaries for Linux on OS X · Graham Enos](https://grahamenos.com/rust-osx-linux-musl.html)
-----------------------------------------------------------------------------------------------------------------------

[Easy Windows and Linux cross-compilers for macOS](https://blog.filippo.io/easy-windows-and-linux-cross-compilers-for-macos/)
-----------------------------------------------------------------------------------------------------------------------------

[Cross-compile and link a static binary on macOS for Linux with cargo and rust - chr4](https://chr4.org/blog/2017/03/15/cross-compile-and-link-a-static-binary-on-macos-for-linux-with-cargo-and-rust/)
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
