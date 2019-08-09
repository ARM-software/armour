FROM japaric/x86_64-unknown-linux-musl:latest

RUN apt-get update && \
	apt-get -y install libssl-dev capnproto musl musl-dev musl-tools
RUN ln -s  /usr/bin/musl-gcc /usr/bin/x86_64-linux-musl-gcc

VOLUME ["/home/vagrant/armour-target", "/home/vagrant/armour-target"]

ENV OPENSSL_DIR=/openssl/
