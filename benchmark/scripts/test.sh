#!/bin/bash

if [ -z "$1" ]; then
  echo "please specify one of the setups:\n baseline   -   armour  -  sozu  -  envoy  -  nginx  -  all"
  exit 1
elif [[ "$1" =~ ^(baseline|sozu|envoy|nginx)$ ]]; then
  ./clean.sh
  ./proxy.sh $1
  ./performance.sh $1 3m
elif [ $1 = "armour" ]; then
  ./clean.sh
  ./armour.sh tcp
  ./performance.sh $1 3m
elif [ $1 = "all" ]; then
  ./test.sh nginx
 # ./test.sh armour tcp
  ./test.sh sozu
  ./test.sh envoy
  ./test.sh baseline
fi
