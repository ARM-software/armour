FROM pihealth_trimmed

RUN apk del .build-deps

COPY blue_hr /blue_hr
COPY listener.py /listener.py
EXPOSE 5000

CMD [ "gunicorn", "--workers", "1", "--bind", ":5000", "listener:app" ]
