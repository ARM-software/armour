FROM pihealth_trimmed

RUN mkdir -p /audio && \
    apk update && \
    apk add pulseaudio pulseaudio-utils g++ ffmpeg && \
    rm -rf /var/lib/apt/lists/*

COPY supervisord.conf /etc/supervisord.conf
COPY asound.conf /etc/asound.conf
COPY ["default.pa", "client.conf", "daemon.conf", "/etc/pulse/"]
COPY listener.py ./listener.py
COPY snowboy /snowboy/

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
