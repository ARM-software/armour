FROM arm32v5/python:latest

RUN apt update \
  && apt install -y --no-install-recommends screen net-tools netcat wget2 curl \
       dnsutils ssh tcpdump nmap lsof ipcalc \
  && rm -rf /var/lib/apt/lists/* \
  && pip3 install Flask requests

EXPOSE 80

WORKDIR /app

