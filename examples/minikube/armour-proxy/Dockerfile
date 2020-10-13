FROM rust
RUN apt-get update && apt-get install -y vim &&\
    apt-get install -y capnproto build-essential pkg-config openssl libssl-dev net-tools &&\
    useradd -m --uid 1337 armour &&\
    usermod -aG sudo armour &&\
    su - armour &&\
    mkdir /home/armour/certificates
WORKDIR /home/armour
COPY src .
RUN cargo build -p armour-host --release &&\
    cargo build -p armour-proxy --release &&\
    cargo build -p armour-ctl --release &&\
    cargo build -p armour-control --release
COPY script.sh sc-egress.sh /home/armour/
RUN chmod 755 /home/armour/script.sh
RUN chmod 755 /home/armour/sc-egress.sh
WORKDIR /home/armour