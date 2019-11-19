#!/bin/bash

function test {
echo Latency >> $1
  echo -e "\nLatency and Throughput using wrk2, each test is run for $3" >> $1
  j=5000
 while [ $j -ge 100 ]
    do
	    echo test$j >> $1
	    echo ./wrk2/wrk -c1 -t1 -R$j -d$3 --latency http://$2 >> $1
      docker exec -it client-$4 ./wrk2/wrk -c1 -t1 -R$j -d$3 --latency  http://$2 >> $1
	sleep 180s
    let "j-=100"
  done
echo Scalability >> $1
  echo -e "\n\nScalability using wrk2 but with n clients per a single server" >> $1
  for j in {1..10}
  do
	  echo test$j >> $1
	 echo  ./wrk2/wrk -c$((j*100)) -t1 -d$3 -R2000 --latency http://$2 >> $1
    docker exec -it client-$4 ./wrk2/wrk -c$((j*100)) -t1 -d$3 -R2000 --latency http://$2 >> $1
sleep 180s
done
}
cd /home/ec2-user/results
dir=$1/
time=$(date +"%Y:%m:%d-%H:%M:%S")
file=$dir$time
echo TCP performance
echo TCP performance >> $file
echo Measuring TCP latency ...

if [ $1 = "armour" ]; then
  srv_nginx=srv-nginx:80
  client=client-4
elif [ $1 = "sozu" ]; then
  srv_nginx=sozu:8080
  client=client-2
elif [ $1 = "envoy" ]; then
  srv_nginx=envoy:1998
  client=client-3
fi

if [ $1 = "baseline" ]; then
  echo -e "\n\nLatency using Qperf" >> $file
  docker exec -d srv-arm qperf

  for i in {1..3}
  do
    echo -e "\n\nQperf: server: srv-arm, client: client-4" >> $file
    docker exec -it client-4 qperf -v -oo msg_size:4:64kib:*2 srv-arm tcp_lat >> $file
  done
elif [[ "$1" =~ ^(armour|sozu|envoy)$ ]]; then
  test $file $srv_nginx $2 $client
fi
echo Measuring HTTP latency for $1 ...
echo HTTP performance >> $file
if [[ "$1" =~ ^(baseline|armour)$ ]]; then
  srv_arm=srv-arm:81
  srv_nginx=srv-nginx:80
  client=client-4
elif [ $1 = "sozu" ]; then
  srv_nginx=localho.st:80
  client=client-2
elif [ $1 = "envoy" ]; then
  srv_nginx=envoy:8080
  client=client-3
elif [ $1 = "nginx" ]; then
  srv_nginx=nginx-proxy:80/nginx
  client=client-1
fi

test $file $srv_nginx $2 $client
echo Done $1
