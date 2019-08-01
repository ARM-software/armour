FROM python:latest

RUN apt update \
  && apt install -y --no-install-recommends screen net-tools netcat wget2 curl \
  dnsutils ssh tcpdump nmap lsof ipcalc \
  && rm -rf /var/lib/apt/lists/* \
  && pip3 install Flask requests

EXPOSE 8080

WORKDIR /armour-home

CMD ["tail", "-f", "/dev/null"]
