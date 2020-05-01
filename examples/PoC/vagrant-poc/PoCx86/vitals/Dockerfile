FROM pihealth_trimmed

ENV BLE_TARGET_BP "c8:fd:19:3a:a2:5f"

#install prerequisites
RUN apk del .build-deps

COPY supervisord.conf /etc/supervisord.conf
COPY blue_bp /blue_bp

#do startscript
CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
