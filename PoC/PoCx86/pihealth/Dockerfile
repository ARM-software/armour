FROM debian

RUN apt-get update && \
    apt-get upgrade && \
    apt-get install wget supervisor python-dev libpython2.7 portaudio19-dev libatlas-base-dev python-pip python-setuptools && \
    pip install --upgrade pip && \
    python -m pip install flask requests pyaudio paho-mqtt

CMD [ "/bin/bash" ]
