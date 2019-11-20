iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6002 -j ACCEPT
sudo iptables -t nat -I PREROUTING -i srv-net+ -p tcp -j DNAT --to-destination 127.0.0.1:6002
sudo iptables -t nat -I PREROUTING -i cl-net+ -p tcp -j DNAT --to-destination 127.0.0.1:6002
for interface in srv-net+ cl-net+; do
iptables -t nat -I PREROUTING -i $interface -p tcp -d 172.19.0.2 --dport 8080 -j DNAT --to-destination 127.0.0.1:6002
iptables -t nat -I PREROUTING -i $interface -p tcp -d 172.20.0.2 --dport 8080 -j DNAT --to-destination 127.0.0.1:6002
iptables -t nat -I PREROUTING -i $interface -p tcp -d 172.22.0.2 --dport 8080 -j DNAT --to-destination 127.0.0.1:6002
iptables -t nat -I PREROUTING -i $interface -p tcp -d 172.21.0.2 --dport 8080 -j DNAT --to-destination 127.0.0.1:6002
done
iptables -t nat -I PREROUTING -m addrtype --dst-type LOCAL -j DOCKER
sudo sysctl -w net.ipv4.conf.srv-net-1.route_localnet=1
sudo sysctl -w net.ipv4.conf.cl-net-1.route_localnet=1
sudo sysctl -w net.ipv4.conf.cl-net-2.route_localnet=1
sudo sysctl -w net.ipv4.ip_forward=1
echo '172.23.0.2 srv-nginx'             >> /etc/hosts
echo '172.21.0.2 client-1'              >> /etc/hosts
echo '172.22.0.2 client-2'      >> /etc/hosts


sudo iptables -t nat -D PREROUTING -p tcp -d 172.23.0.2 --dport 80 -j DNAT --to 127.0.0.1:6002
sudo iptables -t nat -D PREROUTING -p tcp -d 172.22.0.2 --dport 80 -j DNAT --to 127.0.0.1:6002

sudo iptables -t nat -I PREROUTING -p tcp -d 172.23.0.2 --dport 80 -j DNAT --to 172.21.0.2:6002
sudo iptables -t nat -I PREROUTING -p tcp -s 172.21.0.2 --dport 6002 -j DNAT --to 172.22.0.2:80

sudo iptables -t nat -I PREROUTING -i srv-net+ -p tcp -d 172.22.0.2 --dport 80 -j DNAT --to-destination 172.21.0.2:6002
sudo iptables -t nat -I PREROUTING -i srv-net+ -p tcp -d 172.23.0.2 --dport 80 -j DNAT --to-destination 172.21.0.2:6002
sudo iptables -t nat -I PREROUTING -i cl-net+ -p tcp -d 172.22.0.2 --dport 80 -j DNAT --to-destination 172.21.0.2:6002
sudo iptables -t nat -I PREROUTING -i cl-net+ -p tcp -d 172.23.0.2 --dport 80 -j DNAT --to-destination 172.21.0.2:6002


sudo iptables -t nat -I PREROUTING -i srv-net+ -p tcp -s 172.21.0.2 --dport 6002 -j DNAT --to-destination 172.22.0.2:80
sudo iptables -t nat -I PREROUTING -i cl-net+ -p tcp -s 172.21.0.2 --dport 6002 -j DNAT --to-destination 172.22.0.2:80
