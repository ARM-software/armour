#!/usr/bin/env bash
set -e
cd ../../src
ARMOUR_PASS=armour cargo run -p armour-certs -- --dir ../examples/minikube/certificates --control localhost 127.0.0.1 10.0.2.2 $(minikube ip) --host localhost 127.0.0.1 10.0.2.2 $(minikube ip)

scp -i $(minikube ssh-key) -rp armour-init/ docker@$(minikube ip):/home/docker
scp -i $(minikube ssh-key) -rp armour-proxy/ docker@$(minikube ip):/home/docker
scp -i $(minikube ssh-key) -rp certificates docker@$(minikube ip):/home/docker
scp -i $(minikube ssh-key) -rp ../../src/ docker@$(minikube ip):/home/docker/armour-proxy
