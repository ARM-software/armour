FROM alpine:3.8

RUN apk update &&\
    apk upgrade && \
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev && \
    apk add --no-cache bash python2-dev py-pip linux-headers supervisor portaudio-dev && \
    pip install --upgrade pip && \
    pip install --no-cache-dir paho-mqtt requests flask pyaudio gunicorn && \
    apk del .build-deps

RUN mkdir -p /var/log/supervisord && \
    apk add --virtual .builddeps alpine-sdk && \
    python -m pip install --no-cache-dir gunicorn flask-socketio flask-mqtt eventlet gevent-websocket && \
    apk del .builddeps

COPY supervisord.conf /etc/supervisord.conf
COPY webapp ./webapp

ENTRYPOINT [ "/usr/bin/supervisord", "-n", "-c", "/etc/supervisord.conf" ]
