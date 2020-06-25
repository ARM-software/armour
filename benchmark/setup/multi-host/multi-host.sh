#!/bin/bash

function client {
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$1 <<SHELL
cd /home/ec2-user/containers
docker-compose -f client.yml up -d
cd /home/ec2-user/scripts
screen -d -m -S screen-name ./multi-host.sh $2 $3 $4 $5
SHELL
}

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
cd ~/Downloads
aws ec2 run-instances --image-id ami-06f8bcc2bf026bb00 --count 23 --instance-type t2.large --key-name some-key --security-groups "arm-default" "default" --region eu-west-2
sleep 60s
aws ec2 describe-instances --filters "Name=image-id,Values=ami-06f8bcc2bf026bb00"  --region eu-west-2 --query 'Reservations[*].Instances[*].NetworkInterfaces[*].PrivateIpAddresses[*].[Association.PublicIp]' --output text > ips

ARRAY=( $( cat ips ) )
i=1
for ip in "${ARRAY[@]}"
do
  if [ "$i" -le 3 ]; then
  if [ "$i" -eq 1 ]; then
server $ip
srv_public_ip=$ip
server=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 2 ]; then
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
screen
cd /home/ec2-user/scripts
sudo ./add-host.sh $server
sudo rm /etc/nginx/nginx.conf
sudo cp /home/ec2-user/containers/nginx/nginx.conf /etc/nginx/
sudo service nginx start
SHELL
proxy=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 3 ]; then
client $ip nginx $server nan $proxy
fi
elif [ "$i" -le 6 ]; then

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
client $ip envoy $server nan $proxy
fi
elif [ "$i" -le 9 ]; then
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
client $ip linkerd $server nan $proxy
fi
elif [ "$i" -le 11 ]; then
if [ "$i" -eq 10 ]; then
  #server
server $ip
srv_public_ip=$ip
server=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 11 ]; then
client $ip baseline $server
fi
elif [ "$i" -le 14 ]; then
if [ "$i" -eq 12 ]; then
  #server
server $ip
srv_public_ip=$ip
server=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 13 ]; then
  #proxy
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/binaries
screen -d -m -S test-armour ./armour-host --run proxy-allow.conf
SHELL
proxy=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 14 ]; then
  #client
client $ip armour $server allow $proxy
fi
elif [ "$i" -le 17 ]; then
if [ "$i" -eq 15 ]; then
  #server
server $ip
srv_public_ip=$ip
server=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 16 ]; then
  #proxy
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/binaries
screen -d -m -S log ./logger log_sock
cd /home/ec2-user/binaries 
screen -d -m -S test ./armour-host --run proxy-log.conf
SHELL
proxy=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 17 ]; then
  #client
client $ip armour $server log $proxy
fi
 elif [ "$i" -le 20 ]; then
if [ "$i" -eq 18 ]; then
  #server
server $ip
srv_public_ip=$ip
server=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 19 ]; then
  #proxy
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/binaries
screen -d -m -S test-armour ./armour-host --run proxy-all.conf
SHELL
proxy=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 20 ]; then
  #client
client $ip armour $server all $proxy
fi
elif [ "$i" -le 23 ]; then
if [ "$i" -eq 21 ]; then
  #server
server $ip
srv_public_ip=$ip
server=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 22 ]; then
  #proxy
ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/binaries
screen -d -m -S log ./logger log_sock
cd /home/ec2-user/binaries 
screen -d -m -S test ./armour-host --run proxy-all-log.conf
SHELL
proxy=$(aws ec2 describe-instances --filters "Name=ip-address,Values=$ip"   --region eu-west-2 --query 'Reservations[*].Instances[*].[PrivateIpAddress]' --output text)
elif [ "$i" -eq 23 ]; then
  #client
client $ip armour $server all-log $proxy 
fi
fi
i=$((i+1))
done
