Files
=====
- `compose.yml`: compose file with only one single bridge network.
- `docker-compose`: compose file with separate bridge networks for each PoC container and armour-data.
- `generate.sh` script to generate iptables rules and commands for the proxy to execute.
- `remove-generate.sh` script to remove all the added iptables rules.

Requirements
============
- vagrant docker-compose plugin:

		vagrant plugin install vagrant-docker-compose
- vagrant resize disk plugin:

		vagrant plugin install vagrant-disksize

Steps
=====
1. Run vagrant

		vagrant up
		vagrant ssh
2. Generate the iptables rules

		cd /vagrant
		./generate.sh
3. Bash into the `armour-data` container to enforce a policy

		docker exec -it armour-data bash
4. Inside debug container switch to the `/root/` directory and run

		./armour-data-master
	To run the script that opens all the necessary ports and enforces the allow all policy

		run /root/map/proxy_map
At this point, traffic should go through the proxy
5. Get the ip address of the enp0s8 interface, it would be something like 10.1.x.x and test the ip on a browser and 10.1.x.x:81 to see the contracts executing
6. Execute a contract:

		vagrant ssh
	1. Get the debug container ID

			docker ps | grep debug
	2. Bash into the debug container

			docker exec -it <Debug container ID> bash
	3. Inside debug container switch to the `debug_tools/` directory and run:

			./testall.sh

### docker-machine binding (from https://blog.scottlowe.org/2018/01/24/update-on-using-docker-machine-with-vagrant/)
	docker-machine create -d generic \
	--generic-ssh-user vagrant \
	--generic-ssh-key ~/.vagrant.d/insecure_private_key \
	--generic-ssh-port 2222 \
	--generic-ip-address 127.0.0.1 \
	default


	#sudo sed -i 's/app = Flask(__name__)/app = Flask(__name__) \napp.config["SQLALCHEMY_TRACK_MODIFICATIONS"] = False/g' /home/vagrant/PoC/dbread/flask/app.py
	#sudo sed -i 's/app = Flask(__name__)/app = Flask(__name__) \napp.config["SQLALCHEMY_TRACK_MODIFICATIONS"] = False/g' /home/vagrant/PoC/dbwrite/flask/app.py
	#sudo sed -i 's/app = Flask(__name__)/app = Flask(__name__) \napp.config["SQLALCHEMY_TRACK_MODIFICATIONS"] = False/g' /home/vagrant/PoC/verify_id/webapp/listener.py

	docker run -it --net poc_accounting --env MONGO_CONN="mongodb://172.37.0.2" --env MY_NODE_NAME="PoC-armour" poc_accounting

	-p 27017-27019:27017-27019
