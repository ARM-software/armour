
FROM alpine:3.8

RUN apk update &&\
    apk upgrade && \
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev git mariadb-dev
RUN apk add --no-cache bash linux-headers bluez-deprecated
RUN apk add --no-cache bash python2-dev py-pip linux-headers supervisor portaudio-dev && \
    pip install --upgrade pip && \
    pip install mysqlclient flask_sqlalchemy flask_restless enum34 pydub && \
	pip install --no-cache-dir paho-mqtt requests flask pyaudio gunicorn && \
	pip install --no-cache-dir pexpect influxdb jaeger-client opentracing_instrumentation
RUN pip install git+https://git.research.arm.com/rsh/emerge/health-poc/debug-tools.git#egg=debug_tools

RUN  apk add python3 python3-dev && \
     pip3 install --upgrade pip && \
     pip install paho.mqtt && \
     pip install requests && \
     apk del git alpine-sdk glib-dev
RUN pip3 install git+https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git#egg=contract_build_launch

RUN mkdir -p /var/log/supervisord

CMD [ "/bin/bash" ]
