###Files
- `compose.yml`: compose file with only one single bridge network.
- `docker-compose`: compose file with separate bridge networks for each container.

###Requirements
- vagrant docker-compose plugin: 

		vagrant plugin install vagrant-docker-compose
		
###Steps
1. Run vagrant

		vagrant up
2. Get the ip address of the enp0s8 interface, it would be something like 10.1.x.x and test the ip on a browser and 10.1.x.x:81 to see the contracts executing
3. Execute a contract:

		vagrant ssh
	1. Get the debug container ID
	
			docker ps | grep debug
	2. Bash into the debug container
	
			docker exec -it <Debug container ID> bash
	3. Inside debug container switch to the `debug_tools/` directory and run:
	
			./testall.sh 
	