FROM alpine:3.8

RUN apk update &&\
    apk upgrade && \
    apk add --no-cache --virtual .build-deps alpine-sdk glib-dev git && \
    apk add --no-cache bash python2-dev py-pip linux-headers supervisor portaudio-dev && \
    pip install --upgrade pip && \
    pip install --no-cache-dir paho-mqtt requests flask pyaudio gunicorn
RUN pip install git+https://git.research.arm.com/rsh/emerge/health-poc/debug-tools.git#egg=debug_tools

CMD [ "/bin/bash" ]
