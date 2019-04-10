#!/bin/sh

sudo iptables -I FORWARD -i srv-net-1 -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o srv-net-1 -j DOCKER-USER
sudo iptables -I FORWARD -i srv-net-2 -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o srv-net-2 -j DOCKER-USER
sudo iptables -I FORWARD -i srv-net-3 -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o srv-net-3 -j DOCKER-USER
sudo iptables -I FORWARD -i srv-net-4 -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o srv-net-4 -j DOCKER-USER

sudo iptables -I DOCKER-USER -i srv-net-1 -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o srv-net-1 -j ACCEPT
sudo iptables -I DOCKER-USER -i srv-net-2 -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o srv-net-2 -j ACCEPT
sudo iptables -I DOCKER-USER -i srv-net-3 -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o srv-net-3 -j ACCEPT
sudo iptables -I DOCKER-USER -i srv-net-4 -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o srv-net-4 -j ACCEPT

sudo iptables -t nat -I PREROUTING -i srv-net-1 -p tcp -j DNAT --to-destination 10.3.0.2:8443
sudo iptables -t nat -I PREROUTING -i srv-net-2 -p tcp -j DNAT --to-destination 10.3.0.2:8443
sudo iptables -t nat -I PREROUTING -i srv-net-3 -p tcp -j DNAT --to-destination 10.3.0.2:8443
sudo iptables -t nat -I PREROUTING -i srv-net-4 -p tcp -j DNAT --to-destination 10.3.0.2:8443

# Check Masquerading
