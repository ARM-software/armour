#!/usr/bin/env bash
set -e
vagrant up
vagrant ssh -c 'curl https://sh.rustup.rs -sSf | sh -s -- -y'
vagrant ssh -c 'mkdir bin && cd src && cargo build && ln -s ~/src/target/debug/{armour-control,armour-proxy,armour-master,armour-launch,armour-ctl,logger,arm-service} ~/bin'
vagrant ssh -c 'echo "export PATH=\"\$HOME/bin:\$PATH\"" >> ~/.profile'
