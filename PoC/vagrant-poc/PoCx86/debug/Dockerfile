FROM alpine:3.8

RUN apk update &&\
    apk upgrade && \
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev && \
    apk add --no-cache bash python2-dev py-pip linux-headers supervisor portaudio-dev && \
    pip install --upgrade pip && \
    pip install --no-cache-dir paho-mqtt requests flask pyaudio gunicorn && \
    apk del .build-deps

COPY debug_tools ./debug_tools
COPY supervisord.conf /etc/supervisord.conf

RUN  mkdir -p /var/log/supervisord && apk add git alpine-sdk glib-dev mariadb-dev && \
     pip install git+https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git#egg=contract_build_launch && \
     apk del git alpine-sdk glib-dev

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
