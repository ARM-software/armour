#!/bin/bash
cd /home/ec2-user/scripts
sudo ./rules.sh
mkdir /home/ec2-user/results/armour-$1
cd /home/ec2-user/binaries
screen -d -m -S test-armour ./armour-host --run proxy-$1.conf
