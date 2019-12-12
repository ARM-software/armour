#!/bin/bash
cd ~/Downloads
aws ec2 run-instances --image-id ami-0a7e5bed1a9f451f0 --count 48 --instance-type t2.large --key-name some-key --security-groups arm-default --region eu-west-2
sleep 60s
aws ec2 describe-instances --filters "Name=image-id,Values=ami-0a7e5bed1a9f451f0"  --region eu-west-2 --query 'Reservations[*].Instances[*].NetworkInterfaces[*].PrivateIpAddresses[*].[Association.PublicIp]' --output text > ips

ARRAY=( $( cat ips ) )
i=1
for ip in "${ARRAY[@]}"
do
  if [ "$i" -le 4 ]; then
    ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-cherokee scalability
SHELL
elif [ "$i" -le 8 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-cherokee throughput
SHELL
elif [ "$i" -le 12 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-lighttpd throughput
SHELL
elif [ "$i" -le 16 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-lighttpd scalability
SHELL
elif [ "$i" -le 20 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-apache throughput
SHELL
elif [ "$i" -le 24 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-apache scalability
SHELL
elif [ "$i" -le 28 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-actix throughput
SHELL
elif [ "$i" -le 32 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-actix scalability
SHELL
elif [ "$i" -le 36 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-nginx scalability
SHELL
elif [ "$i" -le 40 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-nginx throughput
SHELL
elif [ "$i" -le 44 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-hyper scalability
SHELL
elif [ "$i" -le 48 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./server-test.sh srv-hyper throughput
SHELL
fi
i=$((i+1))
done
