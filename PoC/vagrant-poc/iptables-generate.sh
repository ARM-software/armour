#!/bin/sh

proxy_ip=`docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' armour-data`
interfaces=`ip link | cut -d: -f2 | grep poc`
for i in $interfaces; do
  sudo iptables -I FORWARD -i $i -o proxy-net -j DOCKER-USER
  sudo iptables -I FORWARD -i proxy-net -o $i -j DOCKER-USER

  sudo iptables -I DOCKER-USER -i $i -o proxy-net -j ACCEPT
  sudo iptables -I DOCKER-USER -i proxy-net -o $i -j ACCEPT
done

sudo iptables -t nat -I PREROUTING -i poc_notif -p tcp --dport 80 -j DNAT --to-destination $proxy_ip:5000
sudo iptables -t nat -I PREROUTING -i poc_pulse -p tcp --dport 4713 -j DNAT --to-destination $proxy_ip:5000
sudo iptables -t nat -I PREROUTING -i poc_mongo-web -p tcp --dport 81 -j DNAT --to-destination $proxy_ip:5000

for i in $interfaces; do
  if [[ $i != "poc_mongo-web" && $i != "poc_notif" && $i != "poc_pulse"  $i != "poc_mdebug" && $i != "poc_trust" && $i != "poc_public" ]] then
    sudo iptables -t nat -I PREROUTING -i $i -p tcp -j DNAT --to-destination $proxy_ip:5000
  fi
done

for i in $interfaces; do
  if [[ $i == "poc_mdebug" || $i == "poc_trust" || $i == "poc_public" ]] then
    sudo iptables -t nat -I PREROUTING -i $i -p tcp --match multiport --dport 1883,8883,9001 -j DNAT --to-destination $proxy_ip:5001
  fi
done
