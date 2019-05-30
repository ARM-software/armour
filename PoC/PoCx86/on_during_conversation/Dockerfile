FROM alpine:3.8

RUN apk update &&\
    apk upgrade && \
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev && \
    apk add --no-cache bash python2-dev py-pip linux-headers supervisor portaudio-dev && \
    pip install --upgrade pip && \
    pip install --no-cache-dir paho-mqtt requests flask pyaudio gunicorn && \
    apk del .build-deps

ENV BLE_TARGET_HR "00:22:D0:00:02:09"

#install prerequisites
RUN apk add --no-cache --virtual .build-deps alpine-sdk && \
    apk add --no-cache bash glib-dev linux-headers bluez-deprecated && \
    pip install --no-cache-dir pexpect && \
    mkdir -p /var/log/supervisord && \
    apk del .build-deps

COPY blue_hr /blue_hr
COPY restapi /restapi
COPY supervisord.conf /etc/supervisord.conf

#do startscript
CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
