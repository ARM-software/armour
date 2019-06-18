FROM debian

RUN apt-get update &&\
    apt-get upgrade && \
    apt-get install -y --no-install-recommends bash portaudio19-dev pulseaudio pulseaudio-utils tap-plugins swh-plugins supervisor python3-pip python3 sox && \
    python3 -m pip install --upgrade pip && \
    python3 -m pip install --no-cache-dir requests pulsectl && \
    mkdir -p /var/log/supervisord && \
    apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY supervisord.conf /etc/supervisord.conf
COPY ["default.pa", "daemon.conf", "/etc/pulse/"]
COPY asound.conf /etc/asound.conf
COPY pulse_master.py /pulse_master.py

#do startscript
CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
