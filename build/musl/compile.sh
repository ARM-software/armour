#! /bin/sh

vagrant ssh -c 'cd rust/; cross build --release --target x86_64-unknown-linux-musl'
vagrant ssh -c 'mkdir -p ~/rust/target/docker/bins/'
vagrant ssh -c 'find ~/rust/target/x86_64-unknown-linux-musl/release -maxdepth 1 -type f ! -name "*.*" -exec test -x {} \; -exec cp -ft /home/vagrant/rust/target/docker/bins/ {} +'
