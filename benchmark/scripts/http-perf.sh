#!/bin/bash

function latency {
echo Latency >> $1
echo -e "\nLatency and Throughput using wrk2, each test is run for 60s" >> $1
j=20000
while [ $j -ge 100 ]
do
  echo test$j-$i >> $1
  if [ $3 = "linkerd" ]; then
    echo ./wrk2/wrk -c1 -t1 -R$j -d60s -H "Host: srv-nginx" --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d30s -H "Host: srv-nginx" --latency  http://$2 >> $1
  else 
    echo ./wrk2/wrk -c1 -t1 -R$j -d60s --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d30s --latency  http://$2 >> $1
  fi
  docker restart srv-nginx
  docker restart client-1
  let "j-=500"
done
}

function Scalability {
echo Scalability >> $1
echo -e "\n\nScalability using wrk2 but with n clients per a single server" >> $1
for j in {1..10}
do
  echo test$j >> $1
  if [ $3 = "linkerd" ]; then
    echo  ./wrk2/wrk -c$((j*100)) -t1 -d60s -H "Host: srv-nginx" -R2000 --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c$((j*100)) -t1 -d60s -H "Host: srv-nginx" -R2000 --latency http://$2 >> $1
  else
    echo  ./wrk2/wrk -c$((j*100)) -t1 -d60s -R2000 --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c$((j*100)) -t1 -d60s -R2000 --latency http://$2 >> $1
  fi
done
}

function http {
  echo Measuring HTTP latency for $1 ...
  echo HTTP performance >> $2
  if [[ "$1" =~ ^(baseline|armour)$ ]]; then
    srv=srv-nginx:80
  elif [ $1 = "sozu" ]; then
    srv=localho.st:80
  elif [ $1 = "envoy" ]; then
    srv=$4:8080
  elif [ $1 = "nginx" ]; then
    srv=$4:80/nginx
  elif [ $1 = "linkerd" ]; then
    srv=$4:4140/
  fi
  if [ $3 = "latency" ]; then
    latency $2 $srv $1
  elif [ $3 = "Scalability" ]; then
    Scalability $2 $srv $1
  fi
}
# $1 proxy, $2 latency/scalability, $3 local ip
cd /home/ec2-user/results
if [ $1 = "armour" ]; then
  dir=$1-$4/
else
  dir=$1/
fi
time=$(date +"%Y:%m:%d-%H:%M:%S")
file=$dir$time-$1_$2

if [ $2 = "latency" ]; then
  http $1 $file $2 $3
elif [ $2 = "Scalability" ]; then
  http $1 $file $2 $3
fi

