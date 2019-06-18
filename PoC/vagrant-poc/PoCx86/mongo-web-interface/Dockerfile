FROM alpine:3.8

RUN apk update &&\
    apk upgrade && \
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev && \
    apk add --no-cache bash python2-dev py-pip linux-headers supervisor portaudio-dev && \
    pip install --upgrade pip && \
    pip install --no-cache-dir paho-mqtt requests flask pyaudio gunicorn && \
    apk del .build-deps

RUN  pip install pymongo==3.4.0 flask-pymongo python-dateutil Flask-InfluxDB && mkdir -p /var/log/supervisord

COPY supervisord.conf /etc/supervisord.conf

COPY web-app /web-app

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
