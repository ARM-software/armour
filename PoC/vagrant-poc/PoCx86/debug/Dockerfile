FROM pihealth_trimmed

COPY debug_tools ./debug_tools
COPY supervisord.conf /etc/supervisord.conf

RUN  mkdir -p /var/log/supervisord && apk add git alpine-sdk glib-dev mariadb-dev && \
     apk add python3 && \
     pip3 install --upgrade pip && \
     pip install requests && \
     apk del git alpine-sdk glib-dev

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
