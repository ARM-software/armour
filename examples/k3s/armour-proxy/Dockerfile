FROM ubuntu:18.04
RUN apt-get update && apt-get install -y vim &&\
    apt-get install -y build-essential pkg-config openssl libssl-dev net-tools &&\
    useradd -m --uid 1337 armour &&\
    usermod -aG sudo armour &&\
    su - armour &&\
    mkdir /home/armour/certificates
COPY armour-host armour-proxy script.sh sc-egress.sh /home/armour/
RUN chmod 755 /home/armour/script.sh
RUN chmod 755 /home/armour/sc-egress.sh
WORKDIR /home/armour