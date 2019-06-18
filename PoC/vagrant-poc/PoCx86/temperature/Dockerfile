FROM alpine:latest

RUN apk update &&\
    apk upgrade && \
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev git && \
    apk add --no-cache bash python2-dev py-pip supervisor && \
    pip install --upgrade pip && \
    pip install --no-cache-dir paho-mqtt git+https://git.research.arm.com/rsh/emerge/health-poc/debug-tools.git#egg=debug_tools && \
    apk del .build-deps

COPY read_temp.py /read_temp.py
COPY supervisord.conf /etc/supervisord.conf

RUN  mkdir -p /var/log/supervisord

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
