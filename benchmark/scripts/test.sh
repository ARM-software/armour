#!/bin/bash

ip=`ip a list eth0 | grep -o "inet [0-9]*\.[0-9]*\.[0-9]*\.[0-9]*" | awk '{print$2}'`
sudo sed -i "s/private-ip/$ip/g" /home/ec2-user/containers/docker-compose.yml
if [ -z "$1" ]; then
  echo "please specify one of the setups:\n baseline   -   armour  -  sozu  -  envoy  -  nginx  -  linkerd"
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
elif [ $1 = "linkerd" ]; then
  ./clean.sh
  ./proxy.sh $1
  ./http-perf.sh $1 $2 $ip
elif [ $1 = "armour" ] && [ $3 = "all-async-log" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
elif [ $1 = "armour" ] && [ $3 = "all-log" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
elif [ $1 = "armour" ] && [ $3 = "all" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
elif [ $1 = "armour" ] && [ $3 = "async-log" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
  elif [ $1 = "armour" ] && [ $3 = "log" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
elif [ $1 = "armour" ] && [ $3 = "req-log" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
elif [ $1 = "armour" ] && [ $3 = "req-method" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
elif [ $1 = "armour" ] && [ $3 = "req-res" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
elif [ $1 = "armour" ] && [ $3 = "req" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
elif [ $1 = "armour" ] && [ $3 = "res" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
elif [ $1 = "armour" ] && [ $3 = "srv-payload" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
elif [ $1 = "armour" ] && [ $3 = "allow" ]; then
  ./clean.sh
  ./armour.sh $3
  sleep 60s
  ./http-perf.sh $1 $2 $ip $3
fi
