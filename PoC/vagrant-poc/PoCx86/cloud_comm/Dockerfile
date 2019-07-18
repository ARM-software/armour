FROM golang:alpine as builder
RUN apk update && apk add --no-cache git
WORKDIR $GOPATH/src/cloud-update/app/
COPY listener.go .

RUN go get -d -v
RUN CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -a -v -installsuffix cgo -ldflags="-s" -o /go/bin/listener

############################
# STEP 2 build a small image
############################

FROM scratch
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /go/bin/listener /go/bin/listener
ENTRYPOINT ["/go/bin/listener"]
