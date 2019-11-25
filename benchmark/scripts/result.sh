#!/bin/bash

file=$1
proxy=$2
perc=$3 #ex: 99.000
mkdir /home/ec2-user/results/graphs

function data {
  if [ $1 = "tcp" ]; then
    sed -e '/HTTP perfomance/,$d' "$file" > temp_file
  elif [ $1 = "http" ]; then
    sed -e '1,/HTTP perfomance/d' "$file" > temp_http
  fi
  ofile=/home/ec2-user/results/graphs/$1-$2-$proxy.dat
  if [ $2 = "latency" ]; then
    res=`sed -e '/Scalability/,$d' temp_file | grep -e " read" -e $perc  |  awk 'BEGIN {i=5000} {print i,"  ",$2, "  ", $5; i=i-50}' | awk 'NR%2{printf "%s ",$0;next;}1' | awk '{print $1,"  ",$2,"  ",$5}'`
  elif [ $2 = "scalability" ]; then
    res=`sed -e '1,/Scalability/d' temp_file | grep -e " read" -e $perc  | awk 'BEGIN {i=100} {print i,"  ",$2 ; i=i+50}' | awk 'NR%2{printf "%s ",$0;next;}1' | awk '{print $1,"  ",$2,"  ",$5}'`
  fi
  echo "$res" >> $ofile
}

if [ -z "$1" ]; then
  echo "Usage: ./result.sh [FILE] [PROXY] [PERCENTILE]"
  exit 1
elif [[ "$2" =~ ^(armour|sozu|envoy)$ ]]; then
  data tcp latency
  data tcp scalability
  data http latency
  data http scalability
elif [[ "$2" =~ ^(baseline|nginx)$ ]]; then
  data http latency
  data http scalability
fi

#
#ofile=data-$1
#res=`grep -e "99.000%" -e "Requests/sec:" -e "Transfer/sec:" $1 | awk '{print $2}' | awk '{printf $0 (NR%3?" ":"\n")}'| awk 'BEGIN {i=7000} {print i,"  ",$1 ,"  ",$2,"  ",$3; i=i-200}'`
#echo "$res" >> $ofile
