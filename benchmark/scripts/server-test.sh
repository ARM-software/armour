#!/bin/bash

cd /home/ec2-user/containers
docker-compose -f server-compose.yml up -d
sudo iptables -I DOCKER-USER -j ACCEPT
cd /home/ec2-user/results-server
mkdir $1
dir=$1/
time=$(date +"%Y:%m:%d-%H:%M:%S")
file=$dir$time-$1
if [ $1 = "srv-nginx" ]; then
    srv=srv-nginx:80
elif [ $1 = "srv-apache" ]; then
    srv=srv-apache:80
elif [ $1 = "srv-lighttpd" ]; then
    srv=srv-lighttpd:80
elif [ $1 = "srv-arm" ]; then
    srv=srv-arm:81
elif [ $1 = "srv-cherokee" ]; then
    srv=srv-cherokee:80
fi

j=15000
while [ $j -ge 100 ]
do
i=2501
    while [ $i -ge 1 ]
    do
    echo test$j-$i >> $file
    echo ./wrk2/wrk -c$i -t1 -R$j -d30s --latency http://$srv >> $file
    docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d30s --latency  http://$srv >> $file

  docker restart $1
  docker restart client-1
  let "i-=500"
  done
  let "j-=500"
done