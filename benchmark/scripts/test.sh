#!/bin/bash

ip=`ip a list eth0 | grep -o "inet [0-9]*\.[0-9]*\.[0-9]*\.[0-9]*" | awk '{print$2}'`
sudo sed -i "s/private-ip/$ip/g" /home/ec2-user/containers/docker-compose.yaml
if [ -z "$1" ]; then
  echo "please specify one of the setups:\n baseline   -   armour  -  sozu  -  envoy  -  nginx"
  exit 1
elif [ $1 = "baseline" ]; then
  ./clean.sh
  ./proxy.sh $1
  sudo iptables -I DOCKER-USER -j ACCEPT
  ./http-perf.sh $1 $2 $ip
elif [ $1 = "nginx" ]; then
  ./clean.sh
  ./proxy.sh $1
  ./http-perf.sh $1 $2 $ip
elif [ $1 = "sozu" ]; then
  ./clean.sh
  ./proxy.sh $1
  ./http-perf.sh $1 $2 $ip
elif [ $1 = "envoy" ]; then
  ./clean.sh
  ./proxy.sh $1
  ./http-perf.sh $1 $2 $ip
elif [ $1 = "armour" ] && [ $3 = "log" ]; then
  ./clean.sh
  ./armour.sh log-http
  ./http-perf.sh $1 $2 $ip
elif [ $1 = "armour" ] && [ $3 = "http-req" ]; then
  ./clean.sh
  ./armour.sh http-req
  ./http-perf.sh $1 $2 $ip
elif [ $1 = "armour" ] && [ $3 = "http-id" ]; then
  ./clean.sh
  ./armour.sh http-id
  ./http-perf.sh $1 $2 $ip
elif [ $1 = "armour" ] && [ $3 = "allow" ]; then
  ./clean.sh
  ./armour.sh allow
  ./http-perf.sh $1 $2 $ip
fi
