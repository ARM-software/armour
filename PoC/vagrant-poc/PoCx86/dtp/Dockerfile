FROM alpine:3.8

RUN apk update &&\
    apk upgrade && \
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev && \
    apk add --no-cache bash python2-dev py-pip linux-headers supervisor portaudio-dev && \
    pip install --upgrade pip && \
    pip install --no-cache-dir paho-mqtt requests flask pyaudio gunicorn && \
    apk del .build-deps

RUN mkdir -p /var/log/supervisord && \
    mkdir -p /audio && \
    apk update && \
    apk add pulseaudio pulseaudio-utils g++ ffmpeg git && \
    pip install gunicorn pydub git+https://git.research.arm.com/rsh/emerge/health-poc/debug-tools.git#egg=debug_tools && \
    rm -rf /var/lib/apt/lists/*

COPY supervisord.conf /etc/supervisord.conf
COPY asound.conf /etc/asound.conf
COPY ["default.pa", "client.conf", "daemon.conf", "/etc/pulse/"]
COPY listener.py ./listener.py
COPY snowboy /snowboy/

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
