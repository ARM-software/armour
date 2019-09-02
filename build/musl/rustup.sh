#! /bin/sh

sudo apt-get update
sudo apt-get -y install openssl curl gcc
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > install-rustup.sh
