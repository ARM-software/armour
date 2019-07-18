FROM pihealth_trimmed

RUN mkdir -p /var/log/supervisord && \
    mkdir -p /audio && \
    apk update && \
    apk add pulseaudio pulseaudio-utils g++ ffmpeg git && \
    pip install gunicorn pydub git+https://git.research.arm.com/rsh/emerge/health-poc/debug-tools.git#egg=debug_tools && \
    rm -rf /var/lib/apt/lists/*

COPY supervisord.conf /etc/supervisord.conf
COPY asound.conf /etc/asound.conf
COPY ["default.pa", "client.conf", "daemon.conf", "/etc/pulse/"]
COPY listener.py ./listener.py
COPY snowboy /snowboy/

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]
