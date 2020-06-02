iptables -t nat -I PREROUTING 	       -i poc_+ 	       -p tcp 	       -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.39.0.2 		 --dport 80 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.38.0.2 		 --dport 81 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.35.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.32.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.31.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.30.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.29.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.28.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.24.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.23.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.21.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.20.0.2 		 --dport 6000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.19.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.18.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6000
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.37.0.2 		 --dport 27017 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.34.0.2 		 --dport 4713 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.33.0.2 		 --dport 3306 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.27.0.2 		 --dport 1883 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.26.0.2 		 --dport 1883 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.25.0.2 		 --dport 1883 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.27.0.2 		 --dport 1880 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.26.0.2 		 --dport 1880 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.25.0.2 		 --dport 1880 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.22.0.2 		 --dport 4713 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 		 -i poc_+ 		 -p tcp 		 -d 172.36.0.2 		 --dport 5000 		 -j DNAT --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 	       -i poc_+ 	       -p tcp 	       --dport 53 	       -j DNAT 	       --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 	       -i poc_+ 	       -p tcp 	       --dport 443 	       -j DNAT 	       --to-destination 127.0.0.1:6001
iptables -t nat -I PREROUTING 	       -m addrtype 	       --dst-type LOCAL 	       -j DOCKER
sysctl -w net.ipv4.conf.poc_accounting.route_localnet=1
sysctl -w net.ipv4.conf.poc_colibri.route_localnet=1
sysctl -w net.ipv4.conf.poc_context.route_localnet=1
sysctl -w net.ipv4.conf.poc_conv.route_localnet=1
sysctl -w net.ipv4.conf.poc_dbr.route_localnet=1
sysctl -w net.ipv4.conf.poc_dbw.route_localnet=1
sysctl -w net.ipv4.conf.poc_debug.route_localnet=1
sysctl -w net.ipv4.conf.poc_dtp.route_localnet=1
sysctl -w net.ipv4.conf.poc_launch.route_localnet=1
sysctl -w net.ipv4.conf.poc_mdebug.route_localnet=1
sysctl -w net.ipv4.conf.poc_mongo.route_localnet=1
sysctl -w net.ipv4.conf.poc_mongo-web.route_localnet=1
sysctl -w net.ipv4.conf.poc_mysql.route_localnet=1
sysctl -w net.ipv4.conf.poc_notif.route_localnet=1
sysctl -w net.ipv4.conf.poc_pharm.route_localnet=1
sysctl -w net.ipv4.conf.poc_public.route_localnet=1
sysctl -w net.ipv4.conf.poc_pulse.route_localnet=1
sysctl -w net.ipv4.conf.poc_temp.route_localnet=1
sysctl -w net.ipv4.conf.poc_trust.route_localnet=1
sysctl -w net.ipv4.conf.poc_verify.route_localnet=1
sysctl -w net.ipv4.conf.poc_vitals.route_localnet=1
sysctl -w net.ipv4.conf.poc_cloud.route_localnet=1
sysctl -w net.ipv4.ip_forward=1
iptables -t nat -I POSTROUTING 	       -j MASQUERADE
echo '172.39.0.2 notifications'		>> /etc/hosts
echo '172.38.0.2 mongo-web-interface'	>> /etc/hosts
echo '172.37.0.2 mongo'			>> /etc/hosts
echo '172.36.0.2 cloud-update'		>> /etc/hosts
echo '172.35.0.2 accounting'		>> /etc/hosts
echo '172.34.0.2 context'			>> /etc/hosts
echo '172.33.0.2 mysql'			>> /etc/hosts
echo '172.32.0.2 dbread'			>> /etc/hosts
echo '172.31.0.2 dbwrite'			>> /etc/hosts
echo '172.30.0.2 dtp'			>> /etc/hosts
echo '172.29.0.2 debug'			>> /etc/hosts
echo '172.28.0.2 launch'			>> /etc/hosts
echo '172.27.0.2 mqtt-debug'		>> /etc/hosts
echo '172.26.0.2 mqtt-trusted'		>> /etc/hosts
echo '172.25.0.2 mqtt-public'		>> /etc/hosts
echo '172.24.0.2 picolibri'		>> /etc/hosts
echo '172.23.0.2 pipharm'			>> /etc/hosts
echo '172.22.0.2 pulse'			>> /etc/hosts
echo '172.21.0.2 verify-id'		>> /etc/hosts
echo '172.20.0.2 on-during-conversation'	>> /etc/hosts
echo '172.19.0.2 temperature'		>> /etc/hosts
echo '172.18.0.2 vitals'			>> /etc/hosts