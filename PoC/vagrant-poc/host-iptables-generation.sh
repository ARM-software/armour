#!/bin/bash

# proxy_ip=`docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' armour-data`
rest_port=6000
tcp_port=6001
# rest_endpoints=(172.39.0.2:80 172.38.0.2:81 172.36.0.2:5000 172.35.0.2:5000 172.32.0.2:5000 172.31.0.2:5000 172.30.0.2:5000 172.29.0.2:5000 172.28.0.2:5000 172.24.0.2:5000 172.23.0.2:5000 172.21.0.2:5000 172.20.0.2:6000 172.19.0.2:5000 172.18.0.2:5000)
# tcp_endpoints=172.37.0.2:27017 172.34.0.2:4713 172.33.0.2:3306 172.27.0.2:1883 172.26.0.2:1883 172.25.0.2:1883 172.27.0.2:1880 172.26.0.2:1880 172.25.0.2:1880 172.22.0.2:4713

rest_endpoints=(172.39.0.2:80 172.38.0.2:81 172.35.0.2:5000 172.32.0.2:5000 172.31.0.2:5000 172.30.0.2:5000 172.29.0.2:5000 172.28.0.2:5000 172.24.0.2:5000 172.23.0.2:5000 172.21.0.2:5000 172.20.0.2:6000 172.19.0.2:5000 172.18.0.2:5000)
tcp_endpoints=(172.37.0.2:27017 172.34.0.2:4713 172.33.0.2:3306 172.27.0.2:1883 172.26.0.2:1883 172.25.0.2:1883 172.27.0.2:1880 172.26.0.2:1880 172.25.0.2:1880 172.22.0.2:4713 172.36.0.2:5000)

PROXY_FILE=$1
IPTABLES_FILE=$2

echo $PROXY_FILE
echo $IPTABLES_FILE

rm -f $PROXY_FILE
rm -f $IPTABLES_FILE

mkdir -p "${PROXY_FILE%/*}" && touch "$PROXY_FILE"
mkdir -p "${IPTABLES_FILE%/*}" && touch "$IPTABLES_FILE"

echo "launch" > $PROXY_FILE
echo "wait 1" >> $PROXY_FILE
echo "start "$rest_port >> $PROXY_FILE
# echo "wait 1" >> $PROXY_FILE

# echo "iptables -I FORWARD -i poc_+ -o proxy-net -j DOCKER-USER" > $IPTABLES_FILE
# echo "iptables -I FORWARD -i proxy-net -o poc_+ -j DOCKER-USER" >> $IPTABLES_FILE

# echo "iptables -I DOCKER-USER -i poc_+ -o proxy-net -j ACCEPT" >> $IPTABLES_FILE
# echo "iptables -I DOCKER-USER -i proxy-net -o poc_+ -j ACCEPT" >> $IPTABLES_FILE

# Cloud network is special because it provides the cloud container with internet access
# This rule allows the cloud container to talk to the other contianers through the proxy
echo "iptables -t nat -I PREROUTING -d 172.30.0.0/10 -i cloud -p tcp -j DNAT --to-destination 127.0.0.1:$rest_port" >> $IPTABLES_FILE

for i in "${rest_endpoints[@]}"; do
  IFS=':' read -ra ports <<< "$i"
  echo "iptables -t nat -I PREROUTING -i poc_+ -p tcp -d ${ports[0]} --dport ${ports[1]} -j DNAT --to-destination 127.0.0.1:$rest_port" >> $IPTABLES_FILE
done
echo "iptables -A FORWARD -p tcp -d 127.0.0.1 --dport $rest_port -j ACCEPT" >> $IPTABLES_FILE
echo "iptables -t nat -I PREROUTING -i poc_+ -p tcp -j DNAT --to-destination 127.0.0.1:$rest_port" >> $IPTABLES_FILE

for i in "${tcp_endpoints[@]}"; do
  IFS=':' read -ra ports <<< "$i"
  echo "iptables -t nat -I PREROUTING -i poc_+ -p tcp -d ${ports[0]} --dport ${ports[1]} -j DNAT --to-destination 127.0.0.1:$tcp_port" >> $IPTABLES_FILE
  echo "iptables -A FORWARD -p tcp -d 127.0.0.1 --dport $tcp_port -j ACCEPT" >> $IPTABLES_FILE
  # echo "iptables -A FORWARD -i poc_+ -o lo -j ACCEPT" >> $IPTABLES_FILE
  echo "iptables -A FORWARD  -i lo -m state --state ESTABLISHED,RELATED -j ACCEPT" >> $IPTABLES_FILE
  # echo "iptables -A FORWARD  -i lo -o poc_+ -m state --state ESTABLISHED,RELATED -j ACCEPT" >> $IPTABLES_FILE
  echo "forward "$tcp_port" "$i >> $PROXY_FILE
  # echo "wait 1" >> $PROXY_FILE
  let "tcp_port++"
done
echo "iptables -t nat -I PREROUTING -m addrtype --dst-type LOCAL -j DOCKER" >> $IPTABLES_FILE

echo "allow all" >> $PROXY_FILE

IFACES="poc_accounting poc_colibri poc_context poc_conv poc_dbr poc_dbw poc_debug poc_dtp poc_launch poc_mdebug poc_mongo poc_mongo-web poc_mysql poc_notif poc_pharm poc_public poc_pulse poc_temp poc_trust poc_verify poc_vitals cloud "
for i in $IFACES; do
    echo "sysctl -w net.ipv4.conf.$i.route_localnet=1"  >> $IPTABLES_FILE;
done
echo "sysctl -w net.ipv4.ip_forward=1"  >> $IPTABLES_FILE;


# This rule masquerades the cloud proxy to allow it to talk to the internet
echo "iptables -t nat -I POSTROUTING -s 172.36.0.0/28 ! -o cloud -j MASQUERADE" >> $IPTABLES_FILE

# Allows cloud contaier to move along with DOCKER rules
echo "iptables -I FORWARD -o cloud -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT"  >> $IPTABLES_FILE
echo "iptables -I FORWARD -i cloud -j ACCEPT"  >> $IPTABLES_FILE

echo "echo '172.39.0.2 notifications'		>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.38.0.2 mongo-web-interface'	>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.37.0.2 mongo'			>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.36.0.2 cloud-update'		>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.35.0.2 accounting'		>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.34.0.2 context'			>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.33.0.2 mysql'			>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.32.0.2 dbread'			>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.31.0.2 dbwrite'			>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.30.0.2 dtp'			>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.29.0.2 debug'			>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.28.0.2 launch'			>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.27.0.2 mqtt-debug'		>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.26.0.2 mqtt-trusted'		>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.25.0.2 mqtt-public'		>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.24.0.2 picolibri'		>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.23.0.2 pipharm'			>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.22.0.2 pulse'			>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.21.0.2 verify-id'		>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.20.0.2 on-during-conversation'	>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.19.0.2 temperature'		>> /etc/hosts" >> $IPTABLES_FILE
echo "echo '172.18.0.2 vitals'			>> /etc/hosts" >> $IPTABLES_FILE

# sudo sh -c 'echo "172.39.0.2 notifications"           >> /etc/hosts'
# sudo sh -c 'echo "172.38.0.2 mongo-web-interface"     >> /etc/hosts'
# sudo sh -c 'echo "172.37.0.2 mongo"                   >> /etc/hosts'
# sudo sh -c 'echo "172.36.0.2 cloud-update"            >> /etc/hosts'
# sudo sh -c 'echo "172.35.0.2 accounting"              >> /etc/hosts'
# sudo sh -c 'echo "172.34.0.2 context"                 >> /etc/hosts'
# sudo sh -c 'echo "172.33.0.2 mysql"                   >> /etc/hosts'
# sudo sh -c 'echo "172.32.0.2 dbread"                  >> /etc/hosts'
# sudo sh -c 'echo "172.31.0.2 dbwrite"                 >> /etc/hosts'
# sudo sh -c 'echo "172.30.0.2 dtp"                     >> /etc/hosts'
# sudo sh -c 'echo "172.29.0.2 debug"                   >> /etc/hosts'
# sudo sh -c 'echo "172.28.0.2 launch"                  >> /etc/hosts'
# sudo sh -c 'echo "172.27.0.2 mqtt-debug"              >> /etc/hosts'
# sudo sh -c 'echo "172.26.0.2 mqtt-trusted"            >> /etc/hosts'
# sudo sh -c 'echo "172.25.0.2 mqtt-public"             >> /etc/hosts'
# sudo sh -c 'echo "172.24.0.2 picolibri"               >> /etc/hosts'
# sudo sh -c 'echo "172.23.0.2 pipharm"                 >> /etc/hosts'
# sudo sh -c 'echo "172.22.0.2 pulse"                   >> /etc/hosts'
# sudo sh -c 'echo "172.21.0.2 verify-id"               >> /etc/hosts'
# sudo sh -c 'echo "172.20.0.2 on-during-conversation"  >> /etc/hosts'
# sudo sh -c 'echo "172.19.0.2 temperature"             >> /etc/hosts'
# sudo sh -c 'echo "172.18.0.2 vitals"                  >> /etc/hosts'
