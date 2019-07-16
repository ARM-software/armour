#/bin/sh

DIR=`date +%y_%m_%d_%H_%M`
mkdir -p /armour-playground/pcap-captures/$DIR
tcpdump -G 1800 -v -w /armour-playground/pcap-captures/$DIR/$HOSTNAME.pcap
