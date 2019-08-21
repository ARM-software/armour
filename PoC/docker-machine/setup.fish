function start-poc
    docker-machine create --engine-opt dns-opt=use-vc --engine-opt dns-opt=single-request poc-instance
    eval (docker-machine env poc-instance)
    docker-machine ssh poc-instance -t "tce-load -w -i nginx.tcz"
    docker-machine scp nginx.conf poc-instance:~/
    docker-machine ssh poc-instance -t "sudo cp nginx.conf /usr/local/etc/nginx/nginx.conf"
    docker-machine ssh poc-instance -t "tce-load -w -i dnsmasq.tcz"
    
    mkdir poc-instance
    cd poc-instance

    docker image build -t pihealth_trimmed ../PoCx86/pihealth_trimmed/
    docker image build -t mosquitto ../PoCx86/mosquitto

    # git config --global --unset https.proxy                                                
    git clone https://git.research.arm.com/rsh/emerge/health-poc/accounting.git            
    git clone https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git           
    git clone https://git.research.arm.com/rsh/emerge/health-poc/cloud_comm.git            
    git clone https://git.research.arm.com/rsh/emerge/health-poc/context.git               
    git clone https://git.research.arm.com/rsh/emerge/health-poc/dbread.git                
    git clone https://git.research.arm.com/rsh/emerge/health-poc/dbwrite.git            
    git clone https://git.research.arm.com/rsh/emerge/health-poc/debug.git                 
    git clone https://git.research.arm.com/rsh/emerge/health-poc/digital-dolly.git         
    git clone https://git.research.arm.com/rsh/emerge/health-poc/dtp.git                   
    git clone https://git.research.arm.com/rsh/emerge/health-poc/hostapd.git               
    git clone https://git.research.arm.com/rsh/emerge/health-poc/launch.git                
    git clone https://git.research.arm.com/rsh/emerge/health-poc/mongo-web-interface.git   
    git clone https://git.research.arm.com/rsh/emerge/health-poc/mosquitto_public.git      
    git clone https://git.research.arm.com/rsh/emerge/health-poc/mosquitto_trusted.git     
    git clone https://git.research.arm.com/rsh/emerge/health-poc/notifications.git         
    git clone https://git.research.arm.com/rsh/emerge/health-poc/on_during_conversation.git
    git clone https://git.research.arm.com/rsh/emerge/health-poc/picolibri.git             
    git clone https://git.research.arm.com/rsh/emerge/health-poc/pihealth.git              
    git clone https://git.research.arm.com/rsh/emerge/health-poc/pihealth_trimmed.git      
    git clone https://git.research.arm.com/rsh/emerge/health-poc/pipharm.git               
    git clone https://git.research.arm.com/rsh/emerge/health-poc/pulse.git                 
    git clone https://git.research.arm.com/rsh/emerge/health-poc/temperature.git           
    git clone https://git.research.arm.com/rsh/emerge/health-poc/verify_id.git             
    git clone https://git.research.arm.com/rsh/emerge/health-poc/vitals.git                
    git clone https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git           
    git clone https://git.research.arm.com/rsh/emerge/health-poc/debug-tools.git           

    cp -r ../PoCx86/mysql .
    
    for dir in */
	rm -f $dir/Dockerfile
	set dir $dir
	echo $dir
	cp ../PoCx86/$dir/Dockerfile $dir
    end

    for i in (grep -ril "from queue import Queue" *)
	sed -i -e "s/from queue/from multiprocessing/g" $i
    end
    grep --exclude-dir=rule-engine --exclude='*.yml' --exclude='*.md' -rl . -e 'mysql:/' | \
       xargs sed -i '' -e 's/josh:raspberry@localhost/dbint:raspberry@mysql:3306/g'
    grep --exclude-dir=rule-engine --exclude='*.md' -rl . -e 'mysql://dbint:raspberry@luipen01-rpi.austin.arm.com' | \
       xargs sed -i '' -e 's/dbint:raspberry@luipen01-rpi.austin.arm.com/dbint:raspberry@mysql/g'
    cd ./rule-engine
    grep --exclude-dir=tests -rl . -e 'mysql:/' | xargs sed -i '' -e 's/josh:pocberry@localhost/dbint:raspberry@mysql:3306/g'
    cd ..

    for i in (find . -name '*blue_hr*' -print)
	touch $i/__init__.py
    end
    cp ../docker-compose.yml .
    cp ../compose.yml .
    set -l -x MACHINE_IP (docker-machine ip poc-instance)
    docker-compose -f docker-compose.yml up -d
    docker-machine ssh poc-instance -t "sudo nginx"    
    docker-machine ssh poc-instance -t "sudo dnsmasq"
end
