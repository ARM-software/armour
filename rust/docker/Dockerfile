FROM ubuntu
MAINTAINER Anthony Fox <anthony.fox@arm.com>

ARG BINARY

COPY $BINARY /root

RUN apt update \
  && apt install -y --no-install-recommends build-essential curl ca-certificates libssl-dev pkg-config \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /home
#ENTRYPOINT /root/$BINARY
