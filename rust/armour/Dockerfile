FROM liuchong/rustup:stable

RUN apt update && apt install -y capnproto

WORKDIR /rust/

CMD ["cargo", "build", "--target-dir", "/target/"]
