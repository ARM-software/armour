FROM debian

COPY precise-engine_0.2.0_armv7l.tar.gz /tmp/precise-engine_0.2.0_armv7l.tar.gz

RUN apt-get update &&\
    apt-get upgrade && \
    apt-get install -y --no-install-recommends wget gettext portaudio19-dev supervisor python3-dev python3-pip pulseaudio pulseaudio-utils supervisor python3-setuptools build-essential && \
    pip3 install --no-cache-dir paho-mqtt requests precise-runner && \
    tar xvf /tmp/precise-engine_0.2.0_armv7l.tar.gz && \
    mkdir -p var/log/supervisord && \
    rm -rf /var/lib/apt/lists/* /tmp/precise-engine_0.2.0_armv7l.tar.gz && \
    apt-get purge -y build-essential

COPY asound.conf /etc/asound.conf
COPY ["default.pa", "client.conf", "daemon.conf", "/etc/pulse/"]
COPY supervisord.conf /etc/supervisord.conf
COPY hotword /hotword
COPY startup.sh /startup.sh

ENTRYPOINT [ "/startup.sh" ]
