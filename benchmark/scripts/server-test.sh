#!/bin/bash


function scalability {
j=25000
i=2501
while [ $i -ge 1 ]
do
    echo ./wrk2/wrk -c$i -t1 -R$j -d30s --latency http://$2 >> $3
    docker exec -it client-1 ./wrk2/wrk -c$i -t1 -R$j -d30s --latency  http://$2 >> $3

  docker restart $1
  docker restart client-1
  let "i-=100"
done
} 

function throughput {
j=25000
while [ $j -ge 1 ]
do
    echo ./wrk2/wrk -c1 -t1 -R$j -d30s --latency http://$2 >> $3
    docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d30s --latency  http://$2 >> $3

  docker restart $1
  docker restart client-1
  let "j-=500"
done
}
cd /home/ec2-user/containers
docker-compose -f server-compose.yml up -d
sudo iptables -I DOCKER-USER -j ACCEPT
cd /home/ec2-user/results-server
mkdir $1
dir=$1/
time=$(date +"%Y:%m:%d-%H:%M:%S")
file=$dir$time-$1-$2
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

if [ $2 = "throughput" ]; then
    throughput $1 $srv $file
  elif [ $2 = "scalability" ]; then
    scalability $1 $srv $file
  fi


