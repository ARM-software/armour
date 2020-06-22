#!/usr/bin/env bash
set -e
curl https://sh.rustup.rs -sSf | sh -s -- -y
source $HOME/.cargo/env
cargo install cross
docker build -t arm-unknown-linux-musleabihf .
cd ~/src
cross build --target=arm-unknown-linux-musleabihf -p armour-proxy
cross build --target=arm-unknown-linux-musleabihf -p armour-master
mkdir ~/bin
ln -s ~/src/target/arm-unknown-linux-musleabihf/debug/{armour-proxy,armour-master} ~/bin
