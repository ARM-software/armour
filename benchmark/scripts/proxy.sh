#!/bin/bash

cd /home/ec2-user/results
mkdir $1
echo '172.19.0.2 srv-nginx'             >> /etc/hosts
echo '172.20.0.2 srv-arm'       >> /etc/hosts
echo '172.21.0.2 client-1'              >> /etc/hosts
echo '172.22.0.2 client-2'      >> /etc/hosts
