FROM pihealth_trimmed

RUN apk del .build-deps

COPY webapp ./webapp
COPY supervisord.conf /etc/supervisord.conf

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
