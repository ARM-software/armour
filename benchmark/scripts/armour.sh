#!/bin/bash
cd /home/ec2-user/scripts
sudo ./rules.sh
mkdir /home/ec2-user/results/armour
echo "results for $1" > /home/ec2-user/results/armour/read.me
cd /home/ec2-user/binaries
screen -d -m -S test-armour ./armour-data-master --run proxy-conf-$1.armour
