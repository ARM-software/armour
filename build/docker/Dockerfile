FROM ubuntu
LABEL maintainer="Anthony Fox <anthony.fox@arm.com>"

RUN apt update \
  && apt install -y --no-install-recommends build-essential man zlib1g-dev \
  ca-certificates libssl-dev pkg-config iproute2 net-tools iputils-ping \
  netcat wget2 curl dnsutils ssh tcpdump nmap lsof ipcalc qperf iperf git \
  nginx vim \
#  linux-tools-common linux-tools-generic \
  && rm -rf /var/lib/apt/lists/*

RUN git clone https://github.com/giltene/wrk2.git \
  && cd wrk2 \
  && make

#RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
#  && . $HOME/.cargo/env \
#  && cargo install flamegraph
#ENV PATH="/root/.cargo/bin:$PATH"

WORKDIR /root

COPY bins /usr/local/bin
COPY files /root
#ENTRYPOINT armour-data-master
