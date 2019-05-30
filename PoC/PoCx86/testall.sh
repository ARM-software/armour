#!/bin/bash

GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

result=0


echo "Testing cloud comm"
TEST=`python testcloudupdate.py`
if [ $? -gt 0 ]; then
    echo -e "${RED}Failed cloud comm${NC}"
    let "result += 1"
else
    echo -e "${GREEN}Passed cloud comm${NC}"
fi

echo "Testing dtp"
TEST=`python testdtp.py`
if [ $? -gt 0 ]; then
    echo -e "${RED}Failed dtp${NC}"
    let "result += 1"
else
    echo -e "${GREEN}Passed dtp${NC}"
fi

echo "Testing Verify Id; Registering mock patient id"
TEST=`python testverifyid.py`
if [ $? -gt 0 ]; then
    echo -e "${RED}Failed verify id${NC}"
    let "result += 1"
else
    echo -e "${GREEN}Passed verify id${NC}"
fi

echo "Testing DB Write"
TEST=`python testwritedb.py`
if [ $? -gt 0 ]; then
    echo -e "${RED}Failed db write${NC}"
    let "result += 1"
else
    echo -e "${GREEN}Passed db write${NC}"
fi

echo "Testing DB Read"
TEST=`python testreaddb.py`
if [ $? -gt 0 ]; then
    echo -e "${RED}Failed db read${NC}"
    let "result += 1"
else
    echo -e "${GREEN}Passed db read${NC}"
fi

echo "Testing On During Conversation"
TEST=`python testconvo.py` 
if [ $? -gt 0 ]; then
    echo -e "${RED}Failed convo${NC}"
    let "result += 1"
else
    echo -e "${GREEN}Passed convo${NC}"
fi


if [ $result -gt 0 ]; then
    echo -e "${RED}Failed one or more tests${NC}"
else
    echo -e "${GREEN}Passed All Tests!${NC}"
fi


exit $result
