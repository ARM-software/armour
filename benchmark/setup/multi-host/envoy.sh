#!/bin/bash

function server {
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$1 <<SHELL
screen
cd /home/ec2-user/containers
docker-compose -f server.yml up -d
SHELL
}

function restart-srv { 
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$1 <<SHELL
docker restart srv-hyper
SHELL
}


ARRAY=( $( cat ips-env ) )
cd ~/Downloads
i=4
for ip in "${ARRAY[@]}"
do
if [ "$i" -le 6 ]; then
if [ "$i" -eq 4 ]; then
server $ip
srv_public_ip=$ip
server=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 5 ]; then
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
sudo ./add-host.sh $server
cd /home/ec2-user/containers/envoy
screen -d -m -S envoy ./envoy -c "envoy.yaml"
SHELL
proxy=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 6 ]; then
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/containers
docker-compose -f client.yml up -d
mkdir /home/ec2-user/results/envoy
SHELL
j=25000
while [ $j -ge 100 ]
do
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
echo ./wrk2/wrk -c1 -t1 -R$j -d90s --latency http://$proxy:8080 >> /home/ec2-user/results/envoy/envoy
docker exec client-1 ./wrk2/wrk -c1 -t1 -R$j -d90s --latency  http://$proxy:8080 >> /home/ec2-user/results/envoy/envoy
SHELL
restart-srv $srv_public_ip 
let "j-=1000"
done
fi
fi
i=$((i+1))
done