#!/bin/bash

echo wait 4 > cp
echo $1-in: launch >> cp
echo wait 1 >> cp
echo $1-in: start http $INGRESS >> cp
echo wait 1 >> cp
echo $1-eg: launch >> cp
echo wait 1 >> cp
echo $1-eg: start http $EGRESS >> cp