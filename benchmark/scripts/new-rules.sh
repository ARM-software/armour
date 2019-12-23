iptables -t nat -I PREROUTING -i eth0 -p tcp --dport 80 -j DNAT --to-destination 127.0.0.1:6002
iptables -A FORWARD -p tcp -d 127.0.0.1 --dport 6002 -j ACCEPT

sysctl net.ipv4.ip_forward=1