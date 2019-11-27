#!/bin/bash
cd ~/Downloads
aws ec2 run-instances --image-id ami-0a17cee5e0feb740f --count 35 --instance-type t2.micro --key-name some-key --security-groups arm-default --region eu-west-2
sleep 60s
aws ec2 describe-instances --filters "Name=image-id,Values=ami-0a17cee5e0feb740f"  --region eu-west-2 --query 'Reservations[*].Instances[*].NetworkInterfaces[*].PrivateIpAddresses[*].[Association.PublicIp]' --output text > ips

ARRAY=( $( cat ips ) )
i=1
for ip in "${ARRAY[@]}"
do
  if [ "$i" -le 5 ]; then
    ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
screen
sudo rm /etc/nginx/nginx.conf
sudo cp /home/ec2-user/containers/nginx/nginx.conf /etc/nginx/
sudo service nginx start
cd /home/ec2-user/scripts
screen -d -m -S nginx ./test.sh nginx latency
SHELL
elif [ "$i" -le 10 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/binaries
screen -d -m -S log ./logger log_sock
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency log
SHELL
elif [ "$i" -le 15 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/containers/envoy
screen -d -m -S envoy ./envoy -c "envoy.yaml"
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh envoy latency
SHELL
elif [ "$i" -le 20 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh baseline latency
SHELL
elif [ "$i" -le 25 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency http-req
SHELL
elif [ "$i" -le 30 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency http-id
SHELL
elif [ "$i" -le 35 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency allow
SHELL
fi
i=$((i+1))
done
