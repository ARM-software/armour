#!/bin/bash
cd ../..
mkdir PoC
cd PoC

git clone https://git.research.arm.com/rsh/emerge/health-poc/accounting.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/cloud_comm.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/context.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/dbread.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/dbwrite.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/debug.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/digital-dolly.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/dtp.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/hostapd.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/launch.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/mongo-web-interface.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/mosquitto_public.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/mosquitto_trusted.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/notifications.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/on_during_conversation.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/picolibri.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/pihealth.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/pihealth_trimmed.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/pipharm.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/pulse.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/temperature.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/verify_id.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/vitals.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/rule-engine.git
git clone https://git.research.arm.com/rsh/emerge/health-poc/debug-tools.git

for dir in */
do
  rm -f ${dir}/Dockerfile
  dir=${dir%*/}
  cp ../armour/PoC/PoCx86/${dir}/Dockerfile ${dir}/
done
  cp ../armour/PoC/PoCx86/docker-compose.yml .
