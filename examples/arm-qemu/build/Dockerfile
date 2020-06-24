FROM rustembedded/cross:arm-unknown-linux-musleabihf-0.2.0

RUN apt-get update && \
    apt-get install --assume-yes capnproto && \
    rm -rf /var/lib/apt/lists/*
