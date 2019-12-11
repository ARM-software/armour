#!/bin/bash


function scalability {
j=25000
i=2501
while [ $i -ge 1 ]
do
  id=`pgrep "$4"`
  pid=`echo $id | awk '{print $NF}'`
  screen -d -m -S memory psrecord --interval 1 --log $3_log$i --plot $3_mem$i.png $pid
  echo ./wrk2/wrk -c$i -t1 -R$j -d60s --latency http://$2 >> $3
  docker exec -it client-1 ./wrk2/wrk -c$i -t1 -R$j -d60s --latency  http://$2 >> $3
  docker restart $1
  docker restart client-1
  let "i-=100"
done
} 

function throughput {
j=25000
while [ $j -ge 1 ]
do
  id=`pgrep "$4"`
  pid=`echo $id | awk '{print $NF}'`
  screen -d -m -S memory psrecord --interval 1 --log $3_log$j --plot $3_mem$j.png $pid
  echo ./wrk2/wrk -c1 -t1 -R$j -d60s --latency http://$2 >> $3
  docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d60s --latency  http://$2 >> $3
  docker restart $1
  docker restart client-1
  let "j-=1000"
done
}
cd /home/ec2-user/containers
docker-compose -f server-compose.yml up -d
sudo iptables -I DOCKER-USER -j ACCEPT
cd /home/ec2-user/results-server
mkdir $1
dir=$1/
time=$(date +"%Y:%m:%d-%H:%M:%S")
file=$dir$time-$2
if [ $1 = "srv-nginx" ]; then
    srv=srv-nginx:80
    cmd=nginx
elif [ $1 = "srv-apache" ]; then
    srv=srv-apache:80
    cmd=httpd
elif [ $1 = "srv-lighttpd" ]; then
    srv=srv-lighttpd:80
    cmd=lighttpd
elif [ $1 = "srv-actix" ]; then
    srv=srv-actix
    cmd=actix-server
elif [ $1 = "srv-cherokee" ]; then
    srv=srv-cherokee:80
    cmd=cherokee-wroker
elif [ $1 = "srv-hyper" ]; then
    srv=srv-hyper
    cmd=hyper-server
fi

if [ $2 = "throughput" ]; then
    throughput $1 $srv $file $cmd
  elif [ $2 = "scalability" ]; then
    scalability $1 $srv $file $cmd
  fi


