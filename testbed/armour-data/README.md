Steps
=====
Setup the VM

	vagrant up
	vagrant ssh
	curl https://sh.rustup.rs -sSf | sh
	sudo apt-get -y install openssl
	sudo apt-get install -y libssl-dev
	sudo apt install -y cargo
Clone the armour repo

	git clone https://git.research.arm.com/antfox02/armour.git
Build the armou-data docker images

	cd armour/rust/docker
	./build ~/armour/rust/armour-data-master/ armour-data-master armour-data
Run the docker compose file

	cd /vagrant
	docker-compose up -d
Create the iptable rules

	./iptables-generate.sh
Test
====
To run the test, open 4 different terminal windows and ssh into the vagrant VM:

- Terminal 1:

		docker exec -it armour-data bash
		cd /root/
		./armour-data-master
- Terminal 2:

		docker exec -it armour-data bash
		cd /root/
	- Test HTTP

			./armour-data armour -p 8080
	- Test TCP

			./armour-data armour
		- Terminal 1:

				forward 8080 172.19.0.2:8080
				or
				forward 8080 server-1:8080
- Terminal 3:

		docker exec -it server-1 python3 /flask-server/server.py -d
- Terminal 4:

		docker exec server-2 curl http://172.19.0.2:8080
we should get `request denied`

- Go back to terminal 1 and apply an allow policy:

		allow all
- Try the curl cmd again in terminal 4:

		docker exec server-2 curl http://172.19.0.2:8080

we should get `response`

- To change the policy to deny all, in terminal 1:

		deny all
