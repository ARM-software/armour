iptables -t nat -I PREROUTING -i srv-net+ -p tcp -j DNAT --to-destination 127.0.0.1:6002
iptables -t nat -I PREROUTING -i cl-net+ -p tcp -j DNAT --to-destination 127.0.0.1:6002
sysctl -w net.ipv4.conf.srv-net.route_localnet=1
sysctl -w net.ipv4.conf.cl-net-1.route_localnet=1
sysctl -w net.ipv4.conf.cl-net-2.route_localnet=1
echo '172.18.0.2 server'   >> /etc/hosts
echo '172.19.0.2 client-1' >> /etc/hosts
echo '172.20.0.2 client-2' >> /etc/hosts
