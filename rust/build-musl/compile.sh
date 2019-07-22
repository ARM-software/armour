#! /bin/sh

vagrant ssh -c 'cd rust/; cross build --release --target x86_64-unknown-linux-musl'
