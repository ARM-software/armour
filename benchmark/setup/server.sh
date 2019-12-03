#!/bin/bash
cd ~/Downloads
aws ec2 run-instances --image-id ami-0bc61f3d50e23e1a5 --count 65 --instance-type t2.large --key-name some-key --security-groups arm-default --region eu-west-2
sleep 60s
aws ec2 describe-instances --filters "Name=image-id,Values=ami-0bc61f3d50e23e1a5"  --region eu-west-2 --query 'Reservations[*].Instances[*].NetworkInterfaces[*].PrivateIpAddresses[*].[Association.PublicIp]' --output text > ips

ARRAY=( $( cat ips ) )
i=1
for ip in "${ARRAY[@]}"
do
  if [ "$i" -le 5 ]; then
    ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-cherokee scalability
SHELL
elif [ "$i" -le 10 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-cherokee throughput
SHELL
elif [ "$i" -le 15 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-lighttpd throughput
SHELL
elif [ "$i" -le 20 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-lighttpd scalability
SHELL
elif [ "$i" -le 25 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-apache throughput
SHELL
elif [ "$i" -le 30 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-apache scalability
SHELL
elif [ "$i" -le 35 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-actix throughput
SHELL
elif [ "$i" -le 40 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-actix scalability
SHELL
elif [ "$i" -le 45 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-nginx scalability
SHELL
elif [ "$i" -le 55 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-nginx throughput
SHELL
elif [ "$i" -le 60 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-hyper scalability
SHELL
elif [ "$i" -le 65 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-hyper throughput
SHELL
fi
i=$((i+1))
done
