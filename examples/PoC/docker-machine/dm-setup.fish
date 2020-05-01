#!/usr/local/fish

begin 
    set -l repos   "https://git.research.arm.com/rsh/emerge/health-poc/accounting.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/cloud_comm.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/context.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/dbread.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/dbwrite.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/debug.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/digital-dolly.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/dtp.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/hostapd.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/launch.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/mongo-web-interface.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/mosquitto_public.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/mosquitto_trusted.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/notifications.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/on_during_conversation.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/picolibri.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/pihealth.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/pihealth_trimmed.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/pipharm.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/pulse.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/temperature.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/verify_id.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/vitals.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git" \
		    "https://git.research.arm.com/rsh/emerge/health-poc/debug-tools.git";
		    
    # clone the repos		    
    mkdir new-poc-instance
    for i in $repos
	echo "cloning into " $i
	git -C ./new-poc-instance clone $i
    end

    # if the dockerfile diffs are not there calculate them 
    if ! test -d ./dockerfile-patches
	fish diff2patch.fish
    end

    # apply the patches
    for i in (/bin/ls -1 new-poc-instance/) 
	if test -e ./dockerfile-patches/$i.patch
	    patch -N ./new-poc-instance/$i/Dockerfile ./dockerfile-patches/$i.patch
	end
    end

    #apply specific patches for things not working atm
    patch -N ./new-poc-instance/launch/master.py ./container-patches/launch.patch
    patch -N ./new-poc-instance/debug/debug_tools/testcloudupdate.py ./container-patches/debug.patch

    
    # hack to change hardcoded user names and other failing bits
    begin
	cd new-poc-instance
	for i in (grep -ril "from queue import Queue" *)
	    sed -i -e "s/from queue/from multiprocessing/g" $i
	end
	grep --exclude-dir=rule-engine --exclude='*.yml' --exclude='*.md' -rl . -e 'mysql:/' | \
	    xargs sed -i '' -e 's/josh:raspberry@localhost/dbint:raspberry@mysql:3306/g'
	grep --exclude-dir=rule-engine --exclude='*.md' -rl . -e 'mysql://dbint:raspberry@luipen01-rpi.austin.arm.com' | \
	     xargs sed -i '' -e 's/dbint:raspberry@luipen01-rpi.austin.arm.com/dbint:raspberry@mysql/g'
	begin 
	    cd ./rule-engine
	    grep --exclude-dir=tests -rl . -e 'mysql:/' | xargs sed -i '' -e 's/josh:pocberry@localhost/dbint:raspberry@mysql:3306/g'
	    cd ..
	end
	for i in (find . -name '*blue_hr*' -print)
	    touch $i/__init__.py
	end
	cd ..
    end

    # copy docker-compose files
    cp ./networks-compose.yml ./new-poc-instance/
    cp ./docker-compose.yml ./new-poc-instance/

    # set up docker-machine
    begin
	docker-machine create new-poc-instance
		
	# export port 81 of the VM
	VBoxManage controlvm "new-poc-instance" natpf1 "tcp-port81,tcp,,81,,81";
	set -gx MACHINE_IP (docker-machine ip new-poc-instance)
	# Install dsnmasq and nginx in docker machine, copy nginx.conf and iptables into docker-machine
	docker-machine scp nginx.conf new-poc-instance:~/
	docker-machine scp ./config/iptables-setup.sh new-poc-instance:~/
	docker-machine ssh new-poc-instance -t "sudo install -D nginx.conf /usr/local/etc/nginx/nginx.conf"
	docker-machine ssh new-poc-instance -t "tce-load -w -i dnsmasq.tcz"
	docker-machine ssh new-poc-instance -t "tce-load -w -i nginx.tcz"
	eval (docker-machine env new-poc-instance)
    end

    # start the poc
    begin
	
	docker image build -t mosquitto ./base-images/mosquitto
	docker image build -t mysql ./base-images/mysql
	docker image build -t pihealth_trimmed ./base-images/pihealth_trimmed
	cd ./new-poc-instance/
	docker-compose -p poc -f networks-compose.yml -f docker-compose.yml up -d
	cd ..
    end

    # add google dns
    docker-machine ssh new-poc-instance -t "sudo mv /etc/resolv.conf /etc/resolv.conf.org"
    docker-machine ssh new-poc-instance -t "sudo sh -c \"sed '2inameserver 8.8.4.4' /etc/resolv.conf.org > /etc/resolv.conf\""
    
    # set up iptables 
    begin
	docker-machine ssh new-poc-instance -t "sudo bash iptables-setup.sh"
	set -l command "ln -s $ARMOUR_TARGET_DIR arm"
	docker-machine ssh new-poc-instance -t $command
    end

    # start dnsmasq and nginx in the docker-machine
    docker-machine ssh new-poc-instance -t "sudo dnsmasq && sudo nginx"

    # jump into the VM to run
    docker-machine ssh new-poc-instance
end
