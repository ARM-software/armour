FROM pihealth_trimmed

COPY read_temp.py /read_temp.py
COPY supervisord.conf /etc/supervisord.conf

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
