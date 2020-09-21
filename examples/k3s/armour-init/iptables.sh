#!/bin/bash
iptables -t nat -I PREROUTING 1 -p tcp ! --dport 31000:33000 -j REDIRECT --to-ports $2
iptables -t nat -I OUTPUT 1 -p tcp --dport $1 -m owner --uid-owner 1337 -j DNAT --to 127.0.0.1:$1
iptables -t nat -I OUTPUT 2 -p tcp ! -d 127.0.0.1/32 -m owner ! --uid-owner 1337 -j DNAT --to 127.0.0.1:$3