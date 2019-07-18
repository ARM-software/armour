FROM pihealth_trimmed

COPY listener.py /listener.py
COPY supervisord.conf /etc/supervisord.conf

RUN  mkdir -p /var/log/supervisord

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
