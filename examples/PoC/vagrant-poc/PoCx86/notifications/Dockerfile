FROM pihealth_trimmed

RUN python -m pip install --no-cache-dir flask-socketio flask-mqtt eventlet gevent-websocket && \
    apk del .build-deps

COPY supervisord.conf /etc/supervisord.conf
COPY webapp ./webapp

ENTRYPOINT [ "/usr/bin/supervisord", "-n", "-c", "/etc/supervisord.conf" ]
