#!/bin/bash

proxy_ip=`docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' armour-data`
rest_port=6000
tcp_port=6001
rest_endpoints=(172.39.0.2:80 172.38.0.2:81 172.37.0.2:27017 172.36.0.2:5000 172.35.0.2:5000 172.32.0.2:5000 172.31.0.2:5000 172.30.0.2:5000 172.29.0.2:5000 172.28.0.2:5000 172.24.0.2:5000 172.23.0.2:5000 172.21.0.2:5000 172.20.0.2:6000 172.19.0.2:5000 172.18.0.2:5000)
tcp_endpoints=(172.34.0.2:4713 172.33.0.2:3306 172.27.0.2:1883 172.26.0.2:1883 172.25.0.2:1883 172.27.0.2:1880 172.26.0.2:1880 172.25.0.2:1880 172.22.0.2:4713)
touch map/proxy_map
echo "launch" > map/proxy_map
echo "wait 1" >> map/proxy_map
echo "start "$rest_port >> map/proxy_map
sudo iptables -I FORWARD -i poc_+ -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o poc_+ -j DOCKER-USER

sudo iptables -I DOCKER-USER -i poc_+ -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o poc_+ -j ACCEPT

for i in "${rest_endpoints[@]}"; do
  IFS=':' read -ra ports <<< "$i"
  sudo iptables -t nat -I PREROUTING -i poc_+ -p tcp -d ${ports[0]} --dport ${ports[1]} -j DNAT --to-destination $proxy_ip:$rest_port
  sudo iptables -t nat -I PREROUTING -i poc_+ -p tcp -j DNAT --to-destination $proxy_ip:$rest_port
done

for i in "${tcp_endpoints[@]}"; do
  IFS=':' read -ra ports <<< "$i"
  sudo iptables -t nat -I PREROUTING -i poc_+ -p tcp -d ${ports[0]} --dport ${ports[1]} -j DNAT --to-destination $proxy_ip:$tcp_port
  echo "forward "$tcp_port" "$i >> map/proxy_map
  let "tcp_port++"
done
echo "allow all" >> map/proxy_map
