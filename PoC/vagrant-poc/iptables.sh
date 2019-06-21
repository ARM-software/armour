#!/bin/sh

sudo iptables -I FORWARD -i mysql-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o mysql-net -j DOCKER-USER
sudo iptables -I FORWARD -i notif-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o notif-net -j DOCKER-USER
sudo iptables -I FORWARD -i mongo-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o mongo-net -j DOCKER-USER
sudo iptables -I FORWARD -i cloud-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o cloud-net -j DOCKER-USER
sudo iptables -I FORWARD -i acnt-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o acnt-net -j DOCKER-USER
sudo iptables -I FORWARD -i context-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o context-net -j DOCKER-USER
sudo iptables -I FORWARD -i dbread-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o dbread-net -j DOCKER-USER
sudo iptables -I FORWARD -i dbwrite-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o dbwrite-net -j DOCKER-USER
sudo iptables -I FORWARD -i dtp-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o dtp-net -j DOCKER-USER
sudo iptables -I FORWARD -i debug-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o debug-net -j DOCKER-USER
sudo iptables -I FORWARD -i launch-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o launch-net -j DOCKER-USER
sudo iptables -I FORWARD -i mqttd-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o mqttd-net -j DOCKER-USER
sudo iptables -I FORWARD -i mqtt-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o mqtt-net -j DOCKER-USER
sudo iptables -I FORWARD -i mqttp-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o mqttp-net -j DOCKER-USER
sudo iptables -I FORWARD -i picoli-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o picoli-net -j DOCKER-USER
sudo iptables -I FORWARD -i pipha-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o pipha-net -j DOCKER-USER
sudo iptables -I FORWARD -i pulse-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o pulse-net -j DOCKER-USER
sudo iptables -I FORWARD -i verify-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o verify-net -j DOCKER-USER
sudo iptables -I FORWARD -i during-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o during-net -j DOCKER-USER
sudo iptables -I FORWARD -i temper-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o temper-net -j DOCKER-USER
sudo iptables -I FORWARD -i vitals-net -o proxy-net -j DOCKER-USER
sudo iptables -I FORWARD -i proxy-net -o vitals-net -j DOCKER-USER

sudo iptables -I DOCKER-USER -i mysql-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o mysql-net -j ACCEPT
sudo iptables -I DOCKER-USER -i notif-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o notif-net -j ACCEPT
sudo iptables -I DOCKER-USER -i mongo-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o mongo-net -j ACCEPT
sudo iptables -I DOCKER-USER -i cloud-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o cloud-net -j ACCEPT
sudo iptables -I DOCKER-USER -i acnt-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o acnt-net -j ACCEPT
sudo iptables -I DOCKER-USER -i context-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o context-net -j ACCEPT
sudo iptables -I DOCKER-USER -i dbread-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o dbread-net -j ACCEPT
sudo iptables -I DOCKER-USER -i dbwrite-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o dbwrite-net -j ACCEPT
sudo iptables -I DOCKER-USER -i dtp-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o dtp-net -j ACCEPT
sudo iptables -I DOCKER-USER -i debug-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o debug-net -j ACCEPT
sudo iptables -I DOCKER-USER -i launch-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o launch-net -j ACCEPT
sudo iptables -I DOCKER-USER -i mqttd-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o mqttd-net -j ACCEPT
sudo iptables -I DOCKER-USER -i mqtt-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o mqtt-net -j ACCEPT
sudo iptables -I DOCKER-USER -i mqttp-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o mqttp-net -j ACCEPT
sudo iptables -I DOCKER-USER -i picoli-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o picoli-net -j ACCEPT
sudo iptables -I DOCKER-USER -i pipha-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o pipha-net -j ACCEPT
sudo iptables -I DOCKER-USER -i pulse-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o pulse-net -j ACCEPT
sudo iptables -I DOCKER-USER -i verify-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o verify-net -j ACCEPT
sudo iptables -I DOCKER-USER -i during-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o during-net -j ACCEPT
sudo iptables -I DOCKER-USER -i temper-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o temper-net -j ACCEPT
sudo iptables -I DOCKER-USER -i vitals-net -o proxy-net -j ACCEPT
sudo iptables -I DOCKER-USER -i proxy-net -o vitals-net -j ACCEPT

sudo iptables -t nat -I PREROUTING -i mysql-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i notif-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i mongo-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i cloud-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i acnt-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i context-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i dbread-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i dbwrite-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i dtp-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i debug-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i launch-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i mqttd-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i mqtt-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i mqttp-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i picoli-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i pipha-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i pulse-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i verify-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i during-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
sudo iptables -t nat -I PREROUTING -i temper-net -p tcp -j DNAT --to-destination 172.33.0.2:8443
