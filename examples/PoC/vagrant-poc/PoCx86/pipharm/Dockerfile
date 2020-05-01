FROM pihealth_trimmed

COPY listener.py /listener.py
COPY supervisord.conf /etc/supervisord.conf

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
