FROM pihealth_trimmed

RUN mkdir -p /var/log/supervisord && \
    pip install --upgrade pip && \
    apk add --virtual .builddeps alpine-sdk && \
    python -m pip install --no-cache-dir gunicorn flask-socketio flask-mqtt eventlet gevent-websocket && \
    apk del .builddeps

COPY supervisord.conf /etc/supervisord.conf
COPY webapp ./webapp

ENTRYPOINT [ "/usr/bin/supervisord", "-n", "-c", "/etc/supervisord.conf" ]
