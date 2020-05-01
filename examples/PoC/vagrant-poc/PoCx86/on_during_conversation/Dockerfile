FROM pihealth_trimmed

ENV BLE_TARGET_HR "00:22:D0:00:02:09"

RUN apk del .build-deps

COPY blue_hr /blue_hr
COPY restapi /restapi
COPY supervisord.conf /etc/supervisord.conf

#do startscript
CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
