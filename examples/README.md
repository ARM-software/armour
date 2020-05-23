Setup
=====

The following has been tested with a Mac setup (though this is not essential).

Vagrant
-------

Download and install [Vagrant](https://www.vagrantup.com/downloads.html). By default it will be installed under `/opt/vagrant`. To uninstall follow [these instructions](https://www.vagrantup.com/docs/installation/uninstallation.html).

Alternatively, you can also manage the Vagrant installation using [Homebrew](https://brew.sh).

```shell
% brew cask install vagrant
```

Then install the docker compose plugin

```shell
% vagrant plugin install vagrant-docker-compose
```

Testbed
-------

The following brings up the Vagrant VM

```shell
% cd armour/examples
% ./setup.sh
```

> Note: The VM can be paused, resumed and deleted with:  
> `vagrant pause`  
> `vagrant resume`  
> `vagrant destroy`

Running Armour
--------------

See [data-plane](data-plane.md) for an example of running Armour without a control plane.