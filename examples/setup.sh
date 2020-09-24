#!/usr/bin/env bash
set -e
cd ../src
ARMOUR_PASS=armour cargo run -p armour-certs -- --dir ../examples/certificates --control localhost 127.0.0.1 10.0.2.2 --host localhost 127.0.0.1 10.0.2.2
cd - > /dev/null
cd control-plane
ln -sf ../certificates .
cd - > /dev/null
cd multi-host
ln -sf ../certificates .
cd - > /dev/null
vagrant up
vagrant ssh -c 'curl https://sh.rustup.rs -sSf | sh -s -- -y'
vagrant ssh -c 'rustup toolchain install 1.45.2 && rustup default 1.45.2'
vagrant ssh -c 'mkdir bin && cd src && cargo build && ln -s ~/src/target/debug/{armour-certs,armour-control,armour-proxy,armour-host,armour-launch,armour-ctl,logger,arm-service} ~/bin'
vagrant ssh -c 'echo "export PATH=\"\$HOME/bin:\$PATH\"" >> ~/.profile'
