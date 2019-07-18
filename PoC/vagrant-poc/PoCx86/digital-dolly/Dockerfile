FROM pihealth_trimmed

#install prerequisites
RUN apk add --no-cache --virtual .build-deps alpine-sdk && \
    apk add --no-cache bash glib-dev linux-headers bluez-deprecated && \
    pip install --no-cache-dir pexpect influxdb && \
    apk del .build-deps

COPY blue_hr /blue_hr
COPY listener.py /listener.py
EXPOSE 5000

CMD [ "gunicorn", "--workers", "1", "--bind", ":5000", "listener:app" ]
