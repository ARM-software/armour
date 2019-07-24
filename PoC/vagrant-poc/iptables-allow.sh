#!/bin/sh

interfaces=`ip link | cut -d: -f2 | grep poc`
for i in $interfaces; do
  for j in $interfaces; do
    sudo iptables -I DOCKER-USER -i $i -o $j -j ACCEPT
  done
done
