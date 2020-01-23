#!/bin/bash
cd ~/Downloads
aws ec2 run-instances --image-id ami-0e3f765791acb05e8 --count 60 --instance-type t2.large --key-name some-key --security-groups arm-default --region eu-west-2
sleep 120s
aws ec2 describe-instances --filters "Name=image-id,Values=ami-0e3f765791acb05e8"  --region eu-west-2 --query 'Reservations[*].Instances[*].NetworkInterfaces[*].PrivateIpAddresses[*].[Association.PublicIp]' --output text > ips

ARRAY=( $( cat ips ) )
i=1
for ip in "${ARRAY[@]}"
do
  if [ "$i" -le 3 ]; then
    ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
screen
sudo rm /etc/nginx/nginx.conf
sudo cp /home/ec2-user/containers/nginx/nginx.conf /etc/nginx/
sudo service nginx start
cd /home/ec2-user/scripts
screen -d -m -S nginx ./test.sh nginx latency
SHELL
elif [ "$i" -le 6 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/containers/envoy
screen -d -m -S envoy ./envoy -c "envoy.yaml"
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh envoy latency
SHELL
elif [ "$i" -le 9 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/containers/linkerd/linkerd-1.7.0
screen -d -m -S linkerd ./linkerd-1.7.0-exec config/linkerd.yaml
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh linkerd latency
SHELL
elif [ "$i" -le 12 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh baseline latency
SHELL
elif [ "$i" -le 15 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/binaries
screen -d -m -S log ./logger log_sock
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency all-log
SHELL
elif [ "$i" -le 18 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency all
SHELL
elif [ "$i" -le 21 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/binaries
screen -d -m -S log ./logger log_sock
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency log
SHELL
elif [ "$i" -le 24 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/binaries
screen -d -m -S log ./logger log_sock
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency req-log
SHELL
elif [ "$i" -le 27 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency req-method
SHELL
elif [ "$i" -le 30 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency req-res
SHELL
elif [ "$i" -le 33 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency req
SHELL
elif [ "$i" -le 36 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency res
SHELL
elif [ "$i" -le 39 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency srv-payload
SHELL
elif [ "$i" -le 42 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour latency allow
SHELL
elif [ "$i" -le 45 ]; then
    ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
screen
sudo rm /etc/nginx/nginx.conf
sudo cp /home/ec2-user/containers/nginx/nginx.conf /etc/nginx/
sudo service nginx start
cd /home/ec2-user/scripts
screen -d -m -S nginx ./test.sh nginx Scalability
SHELL
elif [ "$i" -le 48 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/containers/envoy
screen -d -m -S envoy ./envoy -c "envoy.yaml"
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh envoy Scalability
SHELL
elif [ "$i" -le 51 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/containers/linkerd/linkerd-1.7.0
screen -d -m -S linkerd ./linkerd-1.7.0-exec config/linkerd.yaml
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh linkerd Scalability
SHELL
elif [ "$i" -le 54 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh baseline Scalability
SHELL
elif [ "$i" -le 57 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour Scalability allow
SHELL
elif [ "$i" -le 60 ]; then
  ssh -i ~/Downloads/some-key.pem -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no ec2-user@$ip <<SHELL
cd /home/ec2-user/scripts
screen -d -m -S test ./test.sh armour Scalability all
SHELL
fi
i=$((i+1))
done
