#!/bin/bash

file=$1
#proxy=$2
#perc=$3 #ex: 99.000
mkdir /home/ec2-user/results/data

if [ -z "$1" ]; then
  echo "Usage: ./result.sh [FILE]"
  exit 1
fi
ofile=/home/ec2-user/results/data/data-$1
grep -e "0.990625" -e "Requests/sec:" armour_latency | awk '{print $1 " " $2}' | awk '{printf $0 (NR%2?" ":"\n")}'| awk 'BEGIN {i=7000} {print i,"  ",$1 ,"  ",$4; i=i-200}' > $ofile

gawk '{
    vals[$1][$2]
    sum[$1] += $2
    cnt[$1]++
}

END {
    div = 0.3
    for (time in vals) {
        ave  = sum[time] / cnt[time]
        low  = ave * (1 - div)
        high = ave * (1 + div)
        for (val in vals[time]) {
            if ( (val < low) || (val > high) ) {
                print "Deleting outlier", time, val | "cat>&2"
                sum[time] -= val
                cnt[time]--
            }
        }
    }

    for (time in vals) {
        ave = (cnt[time] > 0 ? sum[time] / cnt[time] : 0)
        print time,  ave
    }
}' file_name > output_file
