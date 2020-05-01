Preliminaries
=============

The following assumes a Mac setup (though this is not essential). Note: network (DNS) issues under Vagrant are likely, so it may be necessary to repeat some commands.
For the testbed will be using a vagrant sandbox.

### Docker

Install and run [Docker Desktop](https://www.docker.com/products/docker-desktop). Alternatively, Docker can be installed using Self Service (Jamf).

### Vagrant

Download and install [Vagrant](https://www.vagrantup.com/downloads.html). By default it will be installed under `/opt/vagrant`. To uninstall follow [these instructions](https://www.vagrantup.com/docs/installation/uninstallation.html).

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
    $ cd armour/testbed
    $ vagrant up
    ```

Test
====


1. Run the docker compose file and add the iptables rules

    ```shell
	$ cd /vagrant
	$ docker-compose up -d
	$ ./rules.sh
	```

1. To run the test, open three different terminal windows and ssh into the vagrant VM:

    - Terminal 1 (Armour data plane):

        ```shell
        $ cd binaries
        $ ./armour-master
        ```

        Start an HTTP proxy on port 8080

        ```
        launch log
        http start 6002
        status
        ```

    - Terminal 2 (client):

        ```shell
        $ docker exec client-1 curl http://server:80
        ```

        we should get `request denied`


    - Go back to terminal 1 and apply an allow policy:

        ```shell
        allow all
        ```

    - Try the curl command again in terminal 3:

        ```shell
        $ docker exec server-2 curl http://server-1:80
        ```

        We should now get `response`. We can switch back to blocking with `deny all`.