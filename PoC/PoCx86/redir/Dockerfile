FROM alpine:3.7

USER root

COPY start-supervisor-config.sh /usr/bin

RUN apk add --no-cache musl-dev autoconf automake gettext make gcc git && \
    cd /tmp && \
    git clone https://github.com/troglobit/redir.git && \
    cd redir && git checkout && \
    ./autogen.sh && ./configure --prefix=/usr && \
    make && make install && cd / && rm -rf /tmp/redir && \
    apk del git gcc autoconf make automake make gettext musl-dev && \
    chmod u+x /usr/bin/start-supervisor-config.sh

ENTRYPOINT ["/usr/bin/start-supervisor-config.sh" ]
