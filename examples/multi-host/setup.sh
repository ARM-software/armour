#!/usr/bin/env bash
set -e
vagrant up
vagrant ssh -c 'curl https://sh.rustup.rs -sSf | sh -s -- -y'
vagrant ssh -c 'mkdir bin && cd src && cargo build && ln -s ~/src/target/debug/{armour-proxy,armour-host,armour-launch,armour-ctl} ~/bin'
vagrant ssh -c 'echo "export PATH=\"\$HOME/bin:\$PATH\"" >> ~/.profile'
