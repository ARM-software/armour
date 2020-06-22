#!/usr/bin/env bash
set -e
wget https://github.com/dhruvvyas90/qemu-rpi-kernel/raw/master/kernel-qemu-4.14.79-stretch
wget https://github.com/dhruvvyas90/qemu-rpi-kernel/raw/master/versatile-pb.dtb
wget -O raspbian.zip http://downloads.raspberrypi.org/raspbian_lite/images/raspbian_lite-2018-04-19/2018-04-18-raspbian-stretch-lite.zip
unzip raspbian.zip
qemu-img convert -f raw -O qcow2 2018-04-18-raspbian-stretch-lite.img raspbian.qcow2
cp raspbian.qcow2 raspbian-temp.qcow2
qemu-img resize raspbian.qcow2 +20G
