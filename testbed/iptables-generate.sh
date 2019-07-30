#!/bin/sh

proxy_ip=`docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' armour-data`
interfaces=`ip link | cut -d: -f2 | grep srv`
for i in $interfaces; do
  sudo iptables -I FORWARD -i $i -o proxy-net -j DOCKER-USER
  sudo iptables -I FORWARD -i proxy-net -o $i -j DOCKER-USER

  sudo iptables -I DOCKER-USER -i $i -o proxy-net -j ACCEPT
  sudo iptables -I DOCKER-USER -i proxy-net -o $i -j ACCEPT

  sudo iptables -t nat -I PREROUTING -i $i -p tcp -j DNAT --to-destination $proxy_ip:8080
done
