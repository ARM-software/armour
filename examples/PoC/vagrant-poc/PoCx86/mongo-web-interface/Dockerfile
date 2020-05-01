FROM pihealth_trimmed

RUN  pip install pymongo==3.4.0 flask-pymongo python-dateutil Flask-InfluxDB && mkdir -p /var/log/supervisord
RUN pip uninstall gunicorn
RUN pip3 install gunicorn
COPY supervisord.conf /etc/supervisord.conf

COPY web-app /web-app

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
