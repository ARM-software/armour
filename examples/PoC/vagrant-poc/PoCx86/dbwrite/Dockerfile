FROM pihealth_trimmed

RUN apk add mariadb-connector-c
RUN apk del .build-deps

COPY flask ./flask
COPY supervisord.conf /etc/supervisord.conf

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
