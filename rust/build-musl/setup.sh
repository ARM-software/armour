#! /bin/sh

vagrant up
vagrant ssh -c 'sh install-rustup.sh -y'
vagrant ssh -c 'cargo install cross'
vagrant ssh -c 'cd rust/build-musl; docker build -t musl .'
