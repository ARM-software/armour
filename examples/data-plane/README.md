Data Plane
==========

The following demonstrates the Armour data plane running without a control plane.

### Setup

1. Setup and start the Vagrant VM, see [README](../README.md).
1. Start three terminal windows and in each `ssh` into the vagrant VM:

   ```sh
   host$ cd armour/examples
   host$ vagrant ssh
   ```

	The terminals correspond with the following
	
	1. **Admin**
	1. **Armour data plane**
	1. **Client**

### Example

Perform the following sequence of commands:

1. Start the docker services and apply the `iptables` rules

	**Admin [1]**

	```sh
	vagrant$ cd examples/data-plane
	vagrant$ docker-compose up -d
	vagrant$ sudo ./rules.sh
	```

1. Start the data plane

	**Data plane [2]**

	```sh
	vagrant$ ARMOUR_PASS=password armour-host
	armour-host:> launch log
	armour-host:> start http 6002
	```

1. Make a requests
	
	**Client [3]**
	
	```
   vagrant$ docker exec -ti client-1 curl http://server:80
   request denied
	```

1. Change the policy

	**Data plane [2]**

	```
	armour-host:> allow all
	```

1. Try the request again
	
	**Client [3]**
	
	```
   vagrant$ docker exec -ti client-1 curl http://server:80
   response!
	```
	
1. Stop the services

	**Admin [1]**

	```sh
	vagrant$ docker-compose down
	```

1. Stop the data plane

	**Data plane [2]**

	```
	armour-host:> quit
	```