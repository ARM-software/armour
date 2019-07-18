FROM pihealth_trimmed

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
