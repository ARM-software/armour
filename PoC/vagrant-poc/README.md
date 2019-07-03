**Files**

- `compose.yml`: compose file with only one single bridge network.
- `docker-compose`: compose file with separate bridge networks for each container.

**Requirements**

- vagrant docker-compose plugin: 

		vagrant plugin install vagrant-docker-compose
		
**Steps**

	vagrant up
	// get the ip address of the enp0s8 interface, it would be something like 10.1.x.x
	// test the ip on a browser and 10.1.x.x:81 to see the contracts executing
	//to execute a contract
	vagrant ssh
	// get the debug container ID
	docker ps | grep debug
	// bash into the debug container
	docker exec -it <Debug container ID> bash
	// inside debug container switch to the debug_tools/ directory and run
	./testall.sh 
	