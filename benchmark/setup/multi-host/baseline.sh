#!/bin/bash
# cl 35.178.184.130      172.31.21.170
#srv 35.176.106.99     172.31.16.167
#proxy   3.8.236.237   172.31.31.9
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

ARRAY=( $( cat ips-bas ) )
cd ~/Downloads
i=10
for ip in "${ARRAY[@]}"
do
if [ "$i" -le 11 ]; then
if [ "$i" -eq 10 ]; then
server $ip
srv_public_ip=$ip
server=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 11 ]; then
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/containers
docker-compose -f client.yml up -d
mkdir /home/ec2-user/results/baseline
SHELL
j=25000
while [ $j -ge 100 ]
do
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
echo ./wrk2/wrk -c1 -t1 -R$j -d90s --latency http://$server:80 >> /home/ec2-user/results/baseline/baseline
docker exec client-1 ./wrk2/wrk -c1 -t1 -R$j -d90s --latency  http://$server:80 >> /home/ec2-user/results/baseline/baseline
SHELL
restart-srv $srv_public_ip 
let "j-=1000"
done
fi
fi
i=$((i+1))
done