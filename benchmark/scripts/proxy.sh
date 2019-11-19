#!/bin/bash

cd /home/ec2-user/results
mkdir $1
sudo iptables -I DOCKER-USER -j ACCEPT
