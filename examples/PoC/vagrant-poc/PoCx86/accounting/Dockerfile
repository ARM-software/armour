FROM golang:alpine as builder
RUN apk update && apk add --no-cache git xz curl bash && curl https://raw.githubusercontent.com/golang/dep/master/install.sh | sh
WORKDIR $GOPATH/src/accounting/app/
COPY listener.go .
COPY Gopkg.toml .

RUN dep ensure
RUN curl -LO https://github.com/upx/upx/releases/download/v3.95/upx-3.95-amd64_linux.tar.xz
RUN tar -xvf upx-3.95-amd64_linux.tar.xz
RUN CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -a -v -installsuffix cgo -ldflags="-s" -o /go/bin/listener
RUN ./upx-3.95-amd64_linux/upx /go/bin/listener

############################
# STEP 2 build a small image
############################

#FROM scratch
#COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
#COPY --from=builder /go/bin/listener /go/bin/listener
ENTRYPOINT ["/go/bin/listener"]
