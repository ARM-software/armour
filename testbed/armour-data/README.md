Preliminaries
=============

The following assumes a Mac setup (though this is not essential). Note: network (DNS) issues under Vagrant are likely, so repeating the instructions may be needed.

### Docker

Install and run [Docker Desktop](https://www.docker.com/products/docker-desktop). Alternatively, Docker can be installed using Self Service (Jamf).

### Vargrant

Download and install [Vagrant](https://www.vagrantup.com/downloads.html). By default it will be installed under `/opt/vagrant`. To uninstall follow [these instruction](https://www.vagrantup.com/docs/installation/uninstallation.html).

Alternatively, you can also manage the Vagrant installation using [Homebrew](https://brew.sh).

```shell
$ brew cask install vagrant
```

Testbed Setup
=============

1. Install the docker compose plugin

    ```shell
    $ vagrant plugin install vagrant-docker-compose
    ```

1. Bring up the Vagrant VM

    ```shell
    $ cd armour/testbed/armour-data
    $ vagrant up
    # NOTE: attach to network bridge 1 (en0)
    ```

1. Setup the Vagrant VM for Rust (cargo)

    ```shell
    $ vagrant ssh
    vagrant@ubuntu-bionic:~$ curl https://sh.rustup.rs -sSf | sh -s -- -y
    vagrant@ubuntu-bionic:~$ . .profile
    vagrant@ubuntu-bionic:~$ sudo apt-get -y install libssl-dev
    ```

1. Clone the armour repo.

    ```shell
    vagrant@ubuntu-bionic:~$ git clone https://git.research.arm.com/antfox02/armour.git
    ```

1. Build the armour-data docker images

    ```shell
    vagrant@ubuntu-bionic:~$ cd ~/armour/rust/docker
    vagrant@ubuntu-bionic:~$ ./build ~/armour/rust/armour-data-master armour-data-master armour-data
    ```

Test
====


1. Run the docker compose file

    ```shell
	vagrant@ubuntu-bionic:~$ cd /vagrant
	vagrant@ubuntu-bionic:~$ docker-compose up -d
	```
	
1. Create the iptable rules

    ```shell
   vagrant@ubuntu-bionic:~$ cd ~/armour/testbed/armour-data
	vagrant@ubuntu-bionic:~$ ./iptables-generate.sh
	```
	
1. To run the test, open three different terminal windows and ssh into the vagrant VM:

    - Terminal 1 (Armour data plane):

        ```shell
        vagrant@ubuntu-bionic:~$ docker exec -it armour-data bash
        root@armour-data:~# ./armour-data-master
        ```
        
        Start an HTTP proxy on port 8080
        
        ```
        launch
        start 8080
        ```

    - Terminal 2 (Flask server):

        ```shell
        vagrant@ubuntu-bionic:~$ docker exec -it server-1 python3 /flask-server/server.py -d
        ```

    - Terminal 3 (client):

        ```shell
        vagrant@ubuntu-bionic:~$ docker exec server-2 curl http://server-1:8080
        ```

        we should get `request denied`


    - Go back to terminal 1 and apply an allow policy:

        ```shell
        allow all
        ```

    - Try the curl command again in terminal 3:

        ```shell
        vagrant@ubuntu-bionic:~$ docker exec server-2 curl http://server-1:8080
        ```

        We should now get `response`. We can switch back to blocking with `deny all`.

    - Go back to terminal 1 and switch to TCP proxying:

        ```shell
        stop 8080
        forward 8080 server-1:8080
        ```