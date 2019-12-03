FROM ubuntu
LABEL maintainer="Anthony Fox <anthony.fox@arm.com>"

RUN apt update \
  && apt install -y --no-install-recommends build-essential man zlib1g-dev \
  ca-certificates libssl-dev pkg-config iproute2 net-tools iputils-ping \
  netcat wget2 curl dnsutils ssh tcpdump nmap lsof ipcalc qperf iperf git \
  nginx \
  && rm -rf /var/lib/apt/lists/*

RUN git clone https://github.com/giltene/wrk2.git \
  && cd wrk2 \
  && make

WORKDIR /root

COPY bins /usr/local/bin
#ENTRYPOINT armour-data-master
