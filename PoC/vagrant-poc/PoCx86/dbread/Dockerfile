FROM alpine:3.8

RUN apk update &&\
    apk upgrade && \
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev && \
    apk add --no-cache bash python2-dev py-pip linux-headers supervisor portaudio-dev && \
    pip install --upgrade pip && \
    pip install --no-cache-dir paho-mqtt requests flask pyaudio gunicorn && \
    apk del .build-deps

RUN apk add --no-cache --virtual .build-deps alpine-sdk && \
    apk add mariadb-dev && \
    pip install mysqlclient flask_sqlalchemy flask_restless enum34 && \
    apk del .build-deps

COPY flask ./flask
COPY supervisord.conf /etc/supervisord.conf

RUN mkdir -p /var/log/supervisord

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
