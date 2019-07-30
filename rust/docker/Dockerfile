FROM ubuntu
LABEL maintainer="Anthony Fox <anthony.fox@arm.com>"

RUN apt update \
  && apt install -y --no-install-recommends build-essential man \
  ca-certificates libssl-dev pkg-config iproute2 net-tools iputils-ping \
  netcat wget2 curl dnsutils ssh tcpdump nmap lsof ipcalc rlwrap \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /root

COPY bins /usr/local/bin
#ENTRYPOINT armour-data-master
