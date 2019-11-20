#!/bin/bash
cd /home/ec2-user/scripts
sudo ./rules.sh
mkdir /home/ec2-user/results/armour
cd /home/ec2-user/binaries
screen -d -m -S test-armour ./armour-data-master --run proxy-conf.armour
