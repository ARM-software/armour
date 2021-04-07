FROM ubuntu
RUN apt-get update && apt-get install -y iptables net-tools
ADD iptables.sh /
RUN chmod u+x /iptables.sh