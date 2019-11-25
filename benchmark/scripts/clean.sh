#!/bin/bash

cd /home/ec2-user/containers
docker-compose down
sudo iptables -F
sudo iptables -t nat -F
sudo service docker restart
docker-compose up -d
