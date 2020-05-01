FROM alpine:3.8

RUN apk update &&\
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev && \
    apk add --no-cache bash iproute2 dnsmasq hostapd supervisor && \
    apk del .build-deps && \
    mkdir /var/log/supervisord

COPY dnsmasq.conf /etc/dnsmasq.conf
COPY hostapd.conf /etc/hostapd/hostapd.conf
COPY supervisord.conf /etc/supervisord.conf
COPY start-hostapd.sh /usr/bin/start-hostapd.sh

CMD [ "/usr/bin/start-hostapd.sh" ]
