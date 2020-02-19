FROM alpine:3.8

RUN  mkdir -p /var/log/supervisord && \
     apk add --no-cache python3-dev git alpine-sdk glib-dev supervisor bash mariadb-dev && \
     python3 -m ensurepip && \
     rm -r /usr/lib/python*/ensurepip && \
     pip3 install --upgrade pip setuptools && \
     if [ ! -e /usr/bin/pip ]; then ln -s pip3 /usr/bin/pip ; fi && \
     if [[ ! -e /usr/bin/python ]]; then ln -sf /usr/bin/python3 /usr/bin/python; fi && \
     rm -r /root/.cache && \
     pip3 install git+https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git#egg=contract_build_launch paho-mqtt requests && \
     apk del git alpine-sdk glib-dev

COPY supervisord.conf /etc/supervisord.conf
COPY master.py /master.py

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]



############################
# STEP 1 build application
############################

FROM golang:alpine as builder
RUN apk update && apk add --no-cache git xz curl
COPY src /src
WORKDIR /src
RUN go mod download
RUN curl -LO https://github.com/upx/upx/releases/download/v3.95/upx-3.95-amd64_linux.tar.xz
RUN tar -xvf upx-3.95-amd64_linux.tar.xz
RUN CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -a -v -installsuffix cgo -ldflags="-s" -o /go/bin/launch
RUN ./upx-3.95-amd64_linux/upx /go/bin/launch

############################
# STEP 2 build a small image
############################

FROM scratch
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /go/bin/launch /go/bin/launch
ENTRYPOINT ["/go/bin/launch"]

