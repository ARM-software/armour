#!/bin/sh

function rules {
  sudo iptables -I FORWARD -i $1 -o proxy-net -j DOCKER-USER
  sudo iptables -I FORWARD -i proxy-net -o $1 -j DOCKER-USER

  sudo iptables -I DOCKER-USER -i $1 -o proxy-net -j ACCEPT
  sudo iptables -I DOCKER-USER -i proxy-net -o $1 -j ACCEPT

  sudo iptables -t nat -I PREROUTING -i $1 -p tcp -j DNAT --to-destination $2:8443
}
proxy=`docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' armour-data`
interfaces=`ip link | cut -d: -f2 | grep srv`
for i in $interfaces; do
  rules $i $proxy
done
