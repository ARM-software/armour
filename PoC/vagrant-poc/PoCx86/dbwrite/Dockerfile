FROM pihealth_trimmed

RUN apk add --no-cache --virtual .build-deps alpine-sdk && \
    apk add mariadb-dev && \
    pip install mysqlclient flask_sqlalchemy flask_restless enum34 && \
    apk del .build-deps

COPY flask ./flask
COPY supervisord.conf /etc/supervisord.conf

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
