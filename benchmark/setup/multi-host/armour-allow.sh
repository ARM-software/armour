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


ARRAY=( $( cat ips-arml ) )
cd ~/Downloads
i=12
for ip in "${ARRAY[@]}"
do
if [ "$i" -le 14 ]; then
if [ "$i" -eq 12 ]; then
server $ip
srv_public_ip=$ip
server=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 13 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/binaries
screen -d -m -S test-armour ./armour-master --run proxy-allow.conf
SHELL
proxy=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 14 ]; then
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/containers
docker-compose -f client.yml up -d
mkdir /home/ec2-user/results/armour-allow
srv=$proxy:6002
SHELL
j=25000
while [ $j -ge 100 ]
do
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
echo ./wrk2/wrk -c1 -t1 -R$j -d90s  -H "Host: $server" --latency http://$proxy:6002 >> /home/ec2-user/results/armour-allow/armour
docker exec client-1 ./wrk2/wrk -c1 -t1 -R$j -d90s  -H "Host: $server" --latency  http://$proxy:6002 >> /home/ec2-user/results/armour-allow/armour
SHELL
restart-srv $srv_public_ip 
let "j-=1000"
done
fi
fi
i=$((i+1))
done