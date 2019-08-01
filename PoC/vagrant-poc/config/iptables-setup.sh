iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6000 -j ACCEPT
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.39.0.2 --dport 80 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.38.0.2 --dport 81 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.36.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.35.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.32.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.31.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.30.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.29.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.28.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.24.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.23.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.21.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.20.0.2 --dport 6000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.19.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.18.0.2 --dport 5000 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.34.0.2 --dport 4713 -j DNAT --to-destination 127.0.0.1:6001
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6001 -j ACCEPT
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.27.0.2 --dport 1883 -j DNAT --to-destination 127.0.0.1:6002
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6002 -j ACCEPT
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.26.0.2 --dport 1883 -j DNAT --to-destination 127.0.0.1:6003
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6003 -j ACCEPT
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.25.0.2 --dport 1883 -j DNAT --to-destination 127.0.0.1:6004
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6004 -j ACCEPT
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.27.0.2 --dport 1880 -j DNAT --to-destination 127.0.0.1:6005
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6005 -j ACCEPT
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.26.0.2 --dport 1880 -j DNAT --to-destination 127.0.0.1:6006
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6006 -j ACCEPT
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.25.0.2 --dport 1880 -j DNAT --to-destination 127.0.0.1:6007
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6007 -j ACCEPT
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.22.0.2 --dport 4713 -j DNAT --to-destination 127.0.0.1:6008
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6008 -j ACCEPT
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.33.0.2 --dport 3306 -j DNAT --to-destination 127.0.0.1:6009
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6009 -j ACCEPT
iptables -t nat -I PREROUTING -i poc_+ -p tcp -d 172.37.0.2 --dport 27017 -j DNAT --to-destination 127.0.0.1:6010
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6010 -j ACCEPT
