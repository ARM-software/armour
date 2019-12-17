#!/bin/bash

function latency {
echo Latency >> $1
echo -e "\nLatency and Throughput using wrk2, each test is run for 90s" >> $1
j=25000
if [ $3 != "baseline" ]; then
screen -d -m -S memory psrecord --interval 1 --log $1_log --plot $1_mem.png $4
fi
while [ $j -ge 100 ]
do
  echo test$j >> $1
  if [ $3 = "linkerd" ]; then
    echo ./wrk2/wrk -c1 -t1 -R$j -d90s -H "Host: srv-hyper" --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d90s -H "Host: srv-hyper" --latency  http://$2 >> $1
  else 
    echo ./wrk2/wrk -c1 -t1 -R$j -d90s --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d90s --latency  http://$2 >> $1
  fi
  docker restart srv-hyper
  docker restart client-1
  let "j-=1000"
done
}

function Scalability {
j=2501
if [ $3 != "baseline" ]; then
screen -d -m -S memory psrecord --interval 1 --log $1_log --plot $1_mem.png $4
fi
while [ $j -ge 1 ]
do
  echo test$j >> $1
  if [ $3 = "linkerd" ]; then
    echo ./wrk2/wrk -c$j -t1 -R15000 -d90s -H "Host: srv-hyper" --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c$j -t1 -R15000 -d90s -H "Host: srv-hyper" --latency  http://$2 >> $1
  else 
    echo ./wrk2/wrk -c$j -t1 -R15000 -d90s --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c$j -t1 -R15000 -d90s --latency  http://$2 >> $1
  fi
  docker restart srv-hyper
  docker restart client-1
  let "j-=100"
done
}

function http {
  echo Measuring HTTP latency for $1 ...
  echo HTTP performance >> $2
  if [[ "$1" =~ ^(baseline|armour)$ ]]; then
    srv=srv-nginx:80
    if [ $1 = "armour" ]; then
      cmd=armour-data
    fi
  elif [ $1 = "sozu" ]; then
    srv=localho.st:80
    cmd=sozu
  elif [ $1 = "envoy" ]; then
    srv=$4:8080
    cmd=envoy
  elif [ $1 = "nginx" ]; then
    srv=$4:80/nginx
    cmd=nginx
  elif [ $1 = "linkerd" ]; then
    srv=$4:4140/
    cmd=java
  fi

id=`pgrep "$cmd"`
pid=`echo $id | awk '{print $NF}'`

  if [ $3 = "latency" ]; then
    latency $2 $srv $1 $pid
  elif [ $3 = "Scalability" ]; then
    Scalability $2 $srv $1 $pid
  fi
}
# $1 proxy, $2 latency/scalability, $3 local ip
cd /home/ec2-user/results
if [ $1 = "armour" ]; then
  dir=$1-$4/
else
  dir=$1/
fi
file=$dir$2-$1
http $1 $file $2 $3


