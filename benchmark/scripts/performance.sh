#!/bin/bash

function latency {
echo Latency >> $1
echo -e "\nLatency and Throughput using wrk2, each test is run for 60s" >> $1
j=7000
while [ $j -ge 100 ]
do
  echo test$j-$i >> $1
  echo ./wrk2/wrk -c1 -t1 -R$j -d60s --latency http://$2 >> $1
  docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d30s --latency  http://$2 >> $1
  docker restart srv-nginx
  let "j-=200"
done
}

function Scalability {
echo Scalability >> $1
echo -e "\n\nScalability using wrk2 but with n clients per a single server" >> $1
for j in {1..10}
do
  echo test$j >> $1
  echo  ./wrk2/wrk -c$((j*100)) -t1 -d60s -R2000 --latency http://$2 >> $1
  docker exec -it client-1 ./wrk2/wrk -c$((j*100)) -t1 -d60s -R2000 --latency http://$2 >> $1
done
}

function http {
  echo Measuring HTTP latency for $1 ...
  echo HTTP performance >> $2
  if [[ "$1" =~ ^(baseline|armour)$ ]]; then
    srv_nginx=srv-nginx:80
  elif [ $1 = "sozu" ]; then
    srv_nginx=localho.st:80
  elif [ $1 = "envoy" ]; then
    srv_nginx=$4:8080
  elif [ $1 = "nginx" ]; then
    srv_nginx=$4:80/nginx
  fi
  if [ $3 = "latency" ]; then
    latency $2 $srv_nginx
  elif [ $3 = "Scalability" ]; then
    Scalability $2 $srv_nginx
  fi
}

function tcp {
  echo TCP performance
  echo TCP performance >> $2
  echo Measuring TCP latency ...

  if [ $1 = "armour" ]; then
    srv_nginx=srv-nginx:80
  elif [ $1 = "sozu" ]; then
    srv_nginx=$4:8080
  elif [ $1 = "envoy" ]; then
    srv_nginx=$4:1998
  fi
  if [ $3 = "latency" ]; then
    if [ $1 = "baseline" ]; then
      echo -e "\n\nLatency using Qperf" >> $2
      docker exec -d srv-arm qperf

      for i in {1..3}
      do
        echo -e "\n\nQperf: server: srv-arm, client: client-1" >> $2
        docker exec -it client-1 qperf -v -oo msg_size:4:64kib:*2 srv-arm tcp_lat >> $2
      done
    elif [[ "$1" =~ ^(armour|sozu|envoy)$ ]]; then
      latency $2 $srv_nginx
    fi
  elif [ $3 = "Scalability" ]; then
    Scalability $2 $srv_nginx
  fi
}

cd /home/ec2-user/results
dir=$1/
time=$(date +"%Y:%m:%d-%H:%M:%S")
file=$dir$time-$1_$2_$3

if [ $2 = "tcp" ]; then
  if [ $3 = "latency" ]; then
    tcp $1 $file $3 $4
  elif [ $3 = "Scalability" ]; then
    tcp $1 $file $3 $4
  fi
elif [ $2 = "http" ]; then
  if [ $3 = "latency" ]; then
    http $1 $file $3 $4
  elif [ $3 = "Scalability" ]; then
    http $1 $file $3 $4
  fi
fi
