FROM pihealth_trimmed

COPY debug_tools ./debug_tools
COPY supervisord.conf /etc/supervisord.conf

RUN mkdir -p /var/log/supervisord && apk add python3-dev git alpine-sdk glib-dev mariadb-dev && \
     apk add python3 && \
     pip3 install --upgrade pip && \
     pip install paho.mqtt && \
     pip install requests
RUN pip3 install git+https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git#egg=contract_build_launch && \
     apk del git alpine-sdk glib-dev

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
