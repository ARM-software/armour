FROM alpine:3.8

RUN  mkdir -p /var/log/supervisord && \
     apk add --no-cache python3-dev git alpine-sdk glib-dev supervisor bash mariadb-dev && \
     python3 -m ensurepip && \
     rm -r /usr/lib/python*/ensurepip && \
     pip3 install --upgrade pip setuptools && \
     if [ ! -e /usr/bin/pip ]; then ln -s pip3 /usr/bin/pip ; fi && \
     if [[ ! -e /usr/bin/python ]]; then ln -sf /usr/bin/python3 /usr/bin/python; fi && \
     rm -r /root/.cache && \
     pip3 install git+https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git#egg=contract_build_launch paho-mqtt requests && \
     apk del git alpine-sdk glib-dev

COPY supervisord.conf /etc/supervisord.conf
COPY master.py /master.py

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
