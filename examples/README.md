Setup
=====

The following has been tested with a Mac setup (though this is not essential).

Vagrant
-------

Download and install [Vagrant](https://www.vagrantup.com/downloads.html). By default it will be installed under `/opt/vagrant`. To uninstall follow [these instructions](https://www.vagrantup.com/docs/installation/uninstallation.html).

Alternatively, you can also manage the Vagrant installation using [Homebrew](https://brew.sh).

```shell
host% brew cask install vagrant
```

Then install the docker compose plugin

```shell
host% vagrant plugin install vagrant-docker-compose
```

Setup
-----

The following brings up the Vagrant VM

```shell
host% cd armour/examples
host% ./setup.sh
```

> Note: After the initial setup, the VM can be stopped, started, paused, resumed and deleted with:  
> `host% vagrant halt`  
> `host% vagrant up`  
> `host% vagrant pause`  
> `host% vagrant resume`  
> `host% vagrant destroy`

Armour Examples
---------------

- See [data-plane/](data-plane/README.md) for a minimal example without a control plane.
- See [control-plane/](control-plane/README.md) for an example of Armour running with a control plane.
- See [multi-host/](multi-host/README.md) for an example of Armour running with a distributed setup (two VMs).
- See [arm-qemu/](arm-qemu/README.md) for an example of Armour running on *Raspbian Pi OS* using the QEMU Arm emulator.