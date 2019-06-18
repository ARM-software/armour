FROM alpine:3.8

ENV BLE_TARGET_BP "c8:fd:19:3a:a2:5f"

#install prerequisites
RUN apk add --no-cache --virtual .build-deps alpine-sdk && \
    apk add --no-cache bash glib-dev linux-headers python-dev py-pip supervisor bluez-deprecated && \
    pip install --no-cache-dir bluepy paho-mqtt requests && \
    mkdir -p /var/log/supervisord && \
    apk del .build-deps

COPY supervisord.conf /etc/supervisord.conf
COPY blue_bp /blue_bp

#do startscript
CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
