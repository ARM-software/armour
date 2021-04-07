#!/bin/bash

#!/bin/bash

function latency {
j=25000
while [ $j -ge 100 ]
do
  echo test$j >> $1
  if [ $3 = "linkerd" ]; then
    echo ./wrk2/wrk -c1 -t1 -R$j -d90s -H "Host: srv-hyper" --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d90s -H "Host: srv-hyper" --latency  http://$2 >> $1

 elif [ $3 = "armour" ]; then
    echo ./wrk2/wrk -c1 -t1 -R$j -d90s -H "Host: $4" --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d90s -H "Host: $4" --latency  http://$2 >> $1

  else 
    echo ./wrk2/wrk -c1 -t1 -R$j -d90s --latency http://$2 >> $1
    docker exec -it client-1 ./wrk2/wrk -c1 -t1 -R$j -d90s --latency  http://$2 >> $1
  fi
  let "j-=1000"
done
}
# 1 proxy, 2 server ip, 3 armour stuff, 4 proxy ip

cd /home/ec2-user/results
if [ $1 = "armour" ]; then
  dir=$1-$3/
else
  dir=$1/
fi
mkdir /home/ec2-user/results/$dir
file=$dir$1

if [ -z "$1" ]; then
  echo "please specify one of the setups:\n baseline   -   armour  -  sozu  -  envoy  -  nginx  -  linkerd"
  exit 1
elif [ $1 = "baseline" ]; then
  srv=$2:80
    latency $file $srv $1
elif [ $1 = "nginx" ]; then
  srv=$4:80/hyper
latency $file $srv $1
elif [ $1 = "envoy" ]; then
  srv=$4:8080
latency $file $srv $1
elif [ $1 = "linkerd" ]; then
  srv=$4:4140/
latency $file $srv $1
elif [ $1 = "armour" ] && [ $3 = "all-log" ]; then
  srv=$4:6002
latency $file $srv $1 $2
elif [ $1 = "armour" ] && [ $3 = "all" ]; then
  srv=$4:6002
latency $file $srv $1 $2
elif [ $1 = "armour" ] && [ $3 = "log" ]; then
  srv=$4:6002
latency $file $srv $1 $2
elif [ $1 = "armour" ] && [ $3 = "allow" ]; then
  srv=$4:6002
latency $file $srv $1 $2
fi
