Data Plane
==========

The following demonstrates the Armour data plane running without a control plane.

1. Setup and start the Vagrant VM, see [README](README.md).
1. Setup three terminal windows as follows

	1. **Docker Compose**

   	   ```shell
	   % cd armour/examples
	   % vagrant ssh
	   $ cd examples
	   $ docker-compose up -d
	   $ sudo ./rules.sh
	   ```
	1. **Armour data plane**

	   ```shell
	   % cd armour/examples
	   % vagrant ssh
	   $ ARMOUR_PASS=password armour-master
	   armour-master:> launch log
	   armour-master:> start http 6002
	   ```
   1. **Client**

   	   ```shell
	   % cd armour/examples
	   % vagrant ssh
	   ```
	
1. Update the policy on the *data plane* and test the connections using the *client*. For example

   | **(2) Data Plane** | **(3) Client** |
   |--------------------|------------|
   | | `$ docker exec -ti client-1 curl http://server:80` <br> `request denied` |
   | `armour-master:> allow all` | |
   | | `$ docker exec -ti client-1 curl http://server:80` <br> `response!` |