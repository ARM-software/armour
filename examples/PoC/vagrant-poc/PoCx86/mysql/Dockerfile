FROM mariadb

RUN apt-get update &&\
	apt-get -y upgrade && \
	apt-get -y install  mariadb-client

COPY initdb.sql /docker-entrypoint-initdb.d
