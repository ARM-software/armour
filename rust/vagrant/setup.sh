#! /bin/sh

TARGET_DIR=$1 vagrant up
vagrant ssh -c 'curl https://sh.rustup.rs -sSf | sh -s -- -y'
