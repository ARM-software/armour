#!/usr/local/fish

docker-machine scp nginx.conf new-poc-instance:~/
docker-machine scp ./config/iptables-setup.sh new-poc-instance:~/
docker-machine ssh new-poc-instance -t "sudo install -D nginx.conf /usr/local/etc/nginx/nginx.conf"
docker-machine ssh new-poc-instance -t "tce-load -w -i dnsmasq.tcz"
docker-machine ssh new-poc-instance -t "tce-load -w -i nginx.tcz"

docker-machine ssh new-poc-instance -t "sudo mv /etc/resolv.conf /etc/resolv.conf.org"
docker-machine ssh new-poc-instance -t "sudo sh -c \"sed '2inameserver 8.8.4.4' /etc/resolv.conf.org > /etc/resolv.conf\""
begin
    docker-machine ssh new-poc-instance -t "sudo bash iptables-setup.sh"
    set -l command "ln -s $ARMOUR_TARGET_DIR arm"
    docker-machine ssh new-poc-instance -t $command
end
docker-machine ssh new-poc-instance -t "sudo dnsmasq && sudo nginx"

