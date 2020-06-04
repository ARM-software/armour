Data Plane
==========

The following demonstrates the Armour data plane running without a control plane.

### Setup

1. Setup and start the Vagrant VM, see [README](../README.md).
1. Start three terminal windows and in each `ssh` into the vagrant VM:

   ```shell
   % cd armour/examples
   % vagrant ssh
   ```

	The terminals correspond with the following
	
	1. **Admin**
	1. **Armour data plane**
	1. **Client**

### Example

Perform the following sequence of commands:

1. Start the docker services and apply the `iptables` rules

	**Admin [1]**

	```shell
	$ cd examples/data-plane
	$ docker-compose up -d
	$ sudo ./rules.sh
	```

1. Start the data plane

	**Data plane [2]**

	```shell
	$ ARMOUR_PASS=password armour-master
	armour-master:> launch log
	armour-master:> start http 6002
	```

1. Make a requests
	
	**Client [3]**
	
	```
   $ docker exec -ti client-1 curl http://server:80
   request denied
	```

1. Change the policy

	**Data plane [2]**

	```
	armour-master:> allow all
	```

1. Try the request again
	
	**Client [3]**
	
	```
   $ docker exec -ti client-1 curl http://server:80
   response!
	```
	
1. Stop the services

	**Admin [1]**

	```shell
	$ docker-compose down
	```

1. Stop the data plane

	**Data plane [2]**

	```
	armour-master:> quit
	```