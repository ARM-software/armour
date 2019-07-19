FROM pihealth_trimmed

RUN apk add --no-cache --virtual .build-deps alpine-sdk && \
    pip install flask_sqlalchemy  && \
    apk del .build-deps

RUN mkdir -p /var/log/supervisord

COPY webapp ./webapp
COPY supervisord.conf /etc/supervisord.conf

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
