FROM alpine:3.8

RUN apk update &&\
    apk upgrade && \
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev && \
    apk add --no-cache bash python2-dev py-pip linux-headers supervisor portaudio-dev && \
    pip install --upgrade pip && \
    pip install --no-cache-dir paho-mqtt requests flask pyaudio gunicorn && \
    apk del .build-deps

#install prerequisites
RUN apk add --no-cache bash glib-dev linux-headers bluez-deprecated && \
    pip install --no-cache-dir pexpect influxdb && \
    apk del .build-deps

COPY blue_hr /blue_hr
COPY listener.py /listener.py
EXPOSE 5000

CMD [ "gunicorn", "--workers", "1", "--bind", ":5000", "listener:app" ]
