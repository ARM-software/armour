FROM python:latest

RUN apt update \
  && DEBIAN_FRONTEND=noninteractive apt install -y --no-install-recommends \
       screen net-tools netcat vim wget2 curl dnsutils ssh tcpdump nmap lsof \
       ipcalc qperf iperf iperf3 traceroute openssl build-essential \
  && rm -rf /var/lib/apt/lists/*

RUN git clone https://github.com/wg/wrk.git wrk \
  && cd wrk \
  && make \
  && cd .. \
  && git clone https://github.com/giltene/wrk2.git wrk2 \
  && cd wrk2 \
  && make

EXPOSE 8080
WORKDIR /
COPY hyper-server .
CMD ["tail", "-f", "/dev/null"]
