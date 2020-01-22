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


ARRAY=( $( cat ips-link ) )
cd ~/Downloads
i=7
for ip in "${ARRAY[@]}"
do
if [ "$i" -le 9 ]; then
if [ "$i" -eq 7 ]; then
server $ip
srv_public_ip=$ip
server=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 8 ]; then
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
sudo echo "$server 80" > /home/ec2-user/containers/linkerd/linkerd-1.7.0/disco/srv-hyper
cd /home/ec2-user/containers/linkerd/linkerd-1.7.0
screen -d -m -S linkerd ./linkerd-1.7.0-exec config/linkerd.yaml
SHELL
proxy=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 9 ]; then
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/containers
docker-compose -f client.yml up -d
mkdir /home/ec2-user/results/linkerd
SHELL
sleep 300s
j=25000
while [ $j -ge 100 ]
do
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
echo ./wrk2/wrk -c1 -t1 -R$j -d90s  -H "Host: srv-hyper" --latency http://$proxy:4140/ >> /home/ec2-user/results/linkerd/linkerd
docker exec client-1 ./wrk2/wrk -c1 -t1 -R$j -d90s -H "Host: srv-hyper" --latency  http://$proxy:4140/ >> /home/ec2-user/results/linkerd/linkerd
SHELL
restart-srv $srv_public_ip 
let "j-=1000"
done
fi

fi
i=$((i+1))
done