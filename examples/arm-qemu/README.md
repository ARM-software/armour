## Armour on Arm

The following demos running Armour on a Raspberry Pi emulated using QEMU inside a VM. If you have a Raspberry Pi (running Raspbian Stretch) available then you can skip the **QEMU and Pi setup** section and proceed to installing docker and transferring the Armour binaries to your Pi.
 
### Vagrant Setup

Download and install [Vagrant](https://www.vagrantup.com/downloads.html). For example:

```sh
host% brew cask install vagrant
```

Then bring up a Vagrant VM:

```sh
host% cd examples/arm-demo
host% vagrant up
```

### Build Armour for Arm

The following uses [`cross`](https://github.com/rust-embedded/cross) to build Armour binaries for Arm:

```sh
host% vagrant ssh
vagrant$ cd ~/build
vagrant$ ./setup.sh
```


### QEMU and Pi setup

The following is based on [emulate-raspberry-pi-with-qemu](https://azeria-labs.com/emulate-raspberry-pi-with-qemu/).

- Download a QEMU compatible kernel and an official Raspbian image:

	```sh
	host% vagrant ssh
	vagrant$ cd ~/rpi
	vagrant$ ./setup.sh
	```

- Start QEMU

	```sh
	vagrant$ ./qemu-init.sh
	```
	> Booting may take a while

   and then login at the serial console as user `pi` with password `raspberry`:

	```
	raspberrypi login: pi
	password: raspberry
	```

- Extend the file space on the Pi:

	```sh
	pi$ sudo cfdisk /dev/sdb
	```
	
	delete the second partition (`/dev/sdb2`) and create a `[New]` primary partition with all of the available space. Once new partition is created, use `[Write]` to commit the changes, then `[Quit]` to exit `cfdisk`. 
	
	```sh
	pi$ sudo e2fsck -f /dev/sdb2
	pi$ sudo resize2fs /dev/sdb2
	pi$ sudo halt
	```
	> The kernel will panic after running `halt`, so `killall qemu-system-arm` using another vagrant terminal.

   From now on QEMU can be started with `~/rpi/qemu.sh`.

- Boot the Pi again and enable `ssh`:

	```	sh
	vagrant$ ./qemu.sh
	raspberrypi login: pi
	password: raspberry
	pi$ sudo systemctl enable ssh
	pi$ sudo systemctl start ssh
	```

### Docker setup:

- Install docker:

	> Make sure the Pi is booted first.
	
	```sh
	vagrant$ ssh pi@localhost -p 5555
	pi$ curl -fsSL https://get.docker.com -o get-docker.sh
	pi$ sudo sh get-docker.sh
	pi$ sudo usermod -aG docker pi
	pi$ logout
	```
	> Docker installation may take a while.

- Connect to the Pi again, start docker and check that it is running:

	```sh
	vagrant$ ssh pi@localhost -p 5555	
	pi$ sudo systemctl start docker
	pi$ sudo systemctl status docker
	```

- Install docker compose:

	```sh
	pi$ sudo apt-get install python3-pip
	pi$ sudo pip3 install docker-compose
	```

### Demo

1. Send Armour binaries and examples from the Vagrant VM to the Pi:

	```sh
	vagrant$ scp -P 5555 -rp ~/bin pi@localhost:
	vagrant$ ssh -p 5555 pi@localhost 'mkdir -p arm-demo'
	vagrant$ scp -P 5555 -rp ~/{Dockerfile,server,data-plane} pi@localhost:arm-demo
	```

1. Inside the Pi, start the containers and set the `iptables` rules that will forward all container traffic to Armour's data-plane

	**Terminal 1: Admin**
	
	```	sh
	vagrant$ ssh pi@localhost -p 5555
	pi$ cd ~/arm-demo/data-plane
	pi$ COMPOSE_HTTP_TIMEOUT=100 docker-compose up -d
	pi$ sudo ./rules.sh
	```
	> `docker-compose up` will be slow on the first call as images are pulled and built.

1. Open a second terminal windows and `ssh` into the Pi

	**Terminal 2: Armour data-plane**

	```sh
	host% vagrant ssh
	vagrant$ ssh pi@localhost -p 5555
	pi$ ARMOUR_PASS=password ~/bin/armour-host
	armour-host:> launch log
	armour-host:> start http 6002
	```

2. Make a request

	**Terminal 1: Client**

	```sh
	pi$ docker exec -ti client-1 curl http://server:80
	```
	>you should get: `request denied`

3. Change the policy to allow the traffic

	**Terminal 2: Armour data-plane**

	```
	armour-host:> allow all
	```
		
4. Try the request again

	**Terminal 1: Client**

	```sh
	pi$ docker exec -ti client-1 curl http://server:80
	```
	>you should get: `response!`