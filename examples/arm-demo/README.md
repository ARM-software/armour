## Armour on Arm

We'll run a simple demo application on a single raspberrypi emulated using QEMU inside an Ubuntu VM.

In case you have a Raspberry Pi available, skip the part of setting up QEMU and just transfer Armour component binaries to your pi.
 
### Requirements

Download and install [vagrant](https://www.vagrantup.com/downloads.html).
### Host: Ubuntu VM

Run the following commands too start the VM with Armour src files inside:
> This could take few minutes.

	vagrant up
	vagrant ssh


#### Cross Compile Armour for Arm:

- Install the GNU Arm tooolchain:

        rustup target add arm-unknown-linux-gnueabihf
        sudo apt-get install gcc-arm-linux-gnueabihf
	
- Configure Cargo for cross-compilation:

		mkdir -p ~/.cargo
		
	- Append the following lines to `~/.cargo/config` :
						
			[target.arm-unknown-linux-gnueabihf]
			linker = "arm-linux-gnueabihf-gcc"

	- Build Armour component for Arm:
	
			cd ~/armour
			mkdir bin && cd src
			cargo build --target=arm-unknown-linux-gnueabihf --release
			cp ~/armour/src/target/arm-unknown-linux-gnueabihf/release/{armour-proxy,armour-master} ~/armour/bin
			cp ~/armour/examples/data-plane ~/armour/bin
			

### QEMU set up:
- Create a new folder for the demo

		mkdir ~/rpi-demo && cd ~/rpi-demo

- Installing QEMU:

		sudo apt-get install qemu-system

- Get a QEMU compatible kernel to boot our system

		wget https://github.com/vfdev-5/qemu-rpi2-vexpress/raw/master/kernel-qemu-4.4.1-vexpress
		wget https://github.com/vfdev-5/qemu-rpi2-vexpress/raw/master/vexpress-v2p-ca15-tc1.dtb
- Download the official Raspbian image

		wget -O raspbian_lite_latest.zip https://downloads.raspberrypi.org/raspbian_lite_latest
		unzip raspbian_lite_latest.zip
- Convert it from the raw image to a qcow2 image and add more storage space
>Change the size as you see fit
		
		qemu-img convert -f raw -O qcow2 2020-02-13-raspbian-buster-lite.img rasbian.qcow2
		qemu-img resize rasbian.qcow2 +5G
- start qemu

		qemu-system-arm -m 2048M -M vexpress-a15 -cpu cortex-a15 \
 		 -kernel kernel-qemu-4.4.1-vexpress -no-reboot \
 		 -smp 2 -serial stdio \
 		 -dtb vexpress-v2p-ca15-tc1.dtb -sd rasbian.qcow2 \
 		 -append "root=/dev/mmcblk0p2 rw rootfstype=ext4 console=ttyAMA0,15200 loglevel=8" \
 		 -nic user,hostfwd=tcp::5555-:22
- login at the serial console as user pi with password raspberry
- enable ssh
		
		sudo systemctl enable ssh
- resize partition and filesystem (inside raspi)

		parted /dev/mmcblk0 resizepart 2 100%
		resize2fs /dev/mmcblk0p2

  		
- From your Ubuntu VM you can access the raspberry VM using:
	
		ssh pi@127.0.0.1 -p 5555
	>password: raspberry
		
### Demo setup

- Since the demo uses Docker containers, we'll need to install docker and docker-compose:

		sudo apt-get update && sudo apt-get upgrade
		curl -fsSL https://get.docker.com -o get-docker.sh
		sudo sh get-docker.sh
		sudo usermod -aG docker Pi
		sudo curl -L "https://github.com/docker/compose/releases/download/1.24.0/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
		sudo chmod +x /usr/local/bin/docker-compose
- Send Armour binaries and examples from the ubuntu VM to the pi:

		scp -P 5555 -rp ~/armour/bin pi@127.0.0.1:~/bin

- Inside the Pi: Strat the containers and set the `iptables` rules which will forward all containers traffic to Armour's data-plane
		
		cd ~/bin
		docker-compose up -d
		sudo ./rules.sh
		
- At this point, open 2 terminal windows both ssh inside the Pi:

	1. Terminal 1: Armour data-plane
			
			cd ~/bin
			$ ARMOUR_PASS=password ./armour-master
			armour-master:> launch log
			armour-master:> start http 6002

	2. Terminal 2: Client
	
			docker exec -ti client-1 curl http://server:80
		>you should get: request denied

	3. Terminal 1: Change the policy to allow the traffic
			
			armour-master:> allow all
			
	4. Terminal 2: Try the request again
		
			docker exec -ti client-1 curl http://server:80
		>you should get: response!