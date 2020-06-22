#!/usr/bin/env bash
qemu-system-arm \
  -kernel kernel-qemu-4.14.79-stretch \
  -cpu arm1176 \
  -m 256 \
  -M versatilepb \
  -dtb versatile-pb.dtb \
  -append "root=/dev/sda2 rootfstype=ext4 rw console=ttyAMA0,15200" \
  -hda raspbian-temp.qcow2 \
  -hdb raspbian.qcow2 \
  -no-reboot \
  -nic user,hostfwd=tcp::5555-:22 \
  -nographic
