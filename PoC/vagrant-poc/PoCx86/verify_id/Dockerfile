FROM pihealth_trimmed

RUN mkdir -p /var/log/supervisord && pip install flask-sqlalchemy debug_tools debug

COPY webapp ./webapp
COPY supervisord.conf /etc/supervisord.conf

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
