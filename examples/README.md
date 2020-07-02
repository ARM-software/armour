Armour Examples
===============

The following examples are available:

- [data-plane/](data-plane/README.md) : a minimal example without a control plane.
- [control-plane/](control-plane/README.md) : an example of Armour running with a control plane.
- [multi-host/](multi-host/README.md) : an example of Armour running with a distributed setup (two VMs).
- [arm-qemu/](arm-qemu/README.md) : an example of Armour running on *Raspbian Pi OS* using the QEMU Arm emulator.

Each of these examples has been tested on a Mac (though this is not essential) that is setup as follows.

Vagrant Installation and Setup
-------

Download and install [Vagrant](https://www.vagrantup.com/downloads.html).
> By default Vagrant will be installed under `/opt/vagrant`.  
> To uninstall follow [these instructions](https://www.vagrantup.com/docs/installation/uninstallation.html).

Alternatively, you can install Vagrant using [Homebrew](https://brew.sh):

```shell
host% brew cask install vagrant
```

Then install the docker compose plugin

```shell
host% vagrant plugin install vagrant-docker-compose
```

The following brings up the Vagrant VM. The script also installs `rust` and builds Armour binaries within the VM.

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
