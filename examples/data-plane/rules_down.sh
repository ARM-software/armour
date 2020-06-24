iptables -t nat -D PREROUTING -i srv-net+ -p tcp -j DNAT --to-destination 127.0.0.1:6002
iptables -t nat -D PREROUTING -i cl-net+ -p tcp -j DNAT --to-destination 127.0.0.1:6002
sed -i.bak '/172.18.0.2 server/d' /etc/hosts
sed -i.bak '/172.19.0.2 client-1/d' /etc/hosts
sed -i.bak '/172.20.0.2 client-2/d' /etc/hosts
