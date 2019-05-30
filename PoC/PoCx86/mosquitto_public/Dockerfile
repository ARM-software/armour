FROM alpine:3.7
MAINTAINER David Audet <david.audet@ca.com>

LABEL Description="Eclipse Mosquitto MQTT Broker"

RUN apk update;\
    apk upgrade;\
    apk --no-cache add libssl1.0 libcrypto1.0 libuuid coreutils c-ares-dev openssl-dev musl-dev util-linux-dev git make gcc g++ && \
    git clone https://github.com/eclipse/mosquitto.git && cd mosquitto && git checkout v1.4.12 && \
    make binary && touch /mosquitto/man/mosquitto.8 \
    /mosquitto/man/mosquitto.8 \
    /mosquitto/man/mosquitto.conf.5 \
    /mosquitto/man/mosquitto_passwd.1 \
    /mosquitto/man/mosquitto_pub.1 \
    /mosquitto/man/mosquitto_sub.1 \
    /mosquitto/man/mqtt.7 \
    /mosquitto/man/mosquitto-tls.7 \
    /mosquitto/man/libmosquitto.3 && make DESTDIR= prefix=/usr binary install && \
    apk del coreutils c-ares-dev openssl-dev musl-dev util-linux-dev git make gcc g++ && \
    rm -rf /mosquitto* && mkdir -p /mosquitto/config && \
    addgroup mosquitto && adduser -D -G mosquitto mosquitto && chown -R mosquitto.mosquitto /mosquitto

COPY docker-entrypoint.sh /
COPY mosquitto.conf /mosquitto/config/mosquitto.conf
ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["/usr/sbin/mosquitto", "-c", "/mosquitto/config/mosquitto.conf"]
