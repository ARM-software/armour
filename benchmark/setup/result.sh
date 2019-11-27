#!/bin/bash
mkdir ../results/new/
mkdir ../results/new/armour-log-http ../results/new/armour-http-id ../results/new/armour-http-req ../results/new/armour-allow ../results/new/envoy ../results/new/nginx ../results/new/baseline
cd ../raw-data/
loc=`find . -type f -print0 | xargs -0 ls -t`
i=1
for f in $loc
do
file=`echo $f| cut -f3-4 -d"/" | sed "s/\///"`
if [[ $f == *"armour-allow"* ]]; then
  cp $f ../results/new/armour-allow/$file-$i
elif [[ $f == *"nginx"* ]]; then
cp $f ../results/new/nginx/$file-$i
elif [[ $f == *"envoy"* ]]; then
cp $f ../results/new/envoy/$file-$i
elif [[ $f == *"baseline"* ]]; then
cp $f ../results/new/baseline/$file-$i
elif [[ $f == *"armour-http-req"* ]]; then
cp $f ../results/new/armour-http-req/$file-$i
elif [[ $f == *"armour-http-id"* ]]; then
cp $f ../results/new/armour-http-id/$file-$i
elif [[ $f == *"armour-log-http"* ]]; then
cp $f ../results/new/armour-log-http/$file-$i
fi
i=$((i+1))
done

cd ../results/new/
dir=`ls -d -- */`
for path in $dir
do
    for file in $path/*
        do
        grep -e "0.990625" -e "Requests/sec:" $file | awk '{print $1 " " $2}' | awk '{printf $0 (NR%2?" ":"\n")}'| awk 'BEGIN {i=14000} {print i,"  ",$1 ,"  ",$4; i=i-200}' >> "$path"temp
        done
gawk '{
    vals[$1][$2]
    max[$1]=max[$1]<$2?$2:max[$1]
    min[$1]>$2?$2:min[$1]
    sum[$1] += $2
    cnt[$1]++
    t_sum[$1] += $2
    t_cnt[$1]++
}

END {
    div = 0.3
    for (time in vals) {
        ave  = sum[time] / cnt[time]
        low  = ave * (1 - div)
        high = ave * (1 + div)
        for (val in vals[time]) {
            if ( (val < low) || (val > high) ) {
                t_sum[time] -= val
                t_cnt[time]--
            }
        }
        if ( t_cnt[time] < 1 ) {
            cnt[time] -= 2
            sum[time] -= max[time]
            sum[time] -= min[time]
        }
        else {
            sum[time] = t_sum[time]
            cnt[time] = t_cnt[time]
        }
    }
    for (time in vals) {
        ave = (cnt[time] > 0 ? sum[time] / cnt[time] : 0)
        print time "  " ave
    }
}' "$path"temp > ${path%/}_latency
echo "`cat "${path%/}_latency" | sort -k 1 -n`" >  ${path%/}_latency

gawk '{
    v[$1][$3]
    mx[$1]=mx[$1]<$3?$3:mx[$1]
    mn[$1]>$3?$3:mn[$1]
    sm[$1] += $3
    ct[$1]++
    t_sm[$1] += $3
    t_ct[$1]++
}

END {
    div = 0.3
    for (time in v) {
        ave  = sm[time] / ct[time]
        low  = ave * (1 - div)
        high = ave * (1 + div)
        for (val in v[time]) {
            if ( (val < low) || (val > high) ) {
                t_sm[time] -= val
                t_ct[time]--
            }
        }
        if ( t_ct[time] < 1 ) {
            ct[time] -= 2
            sm[time] -= mx[time]
            sm[time] -= mn[time]
        }
        else {
            sm[time] = t_sm[time]
            ct[time] = t_ct[time]
        }
    }
    for (time in v) {
        ave = (ct[time] > 0 ? sm[time] / ct[time] : 0)
        print time "  " ave
    }
}' "$path"temp > ${path%/}_throughput
echo "`cat "${path%/}_throughput" | sort -k 1 -n`" >  ${path%/}_throughput
rm "$path"temp
done
cd ../results/new
gnuplot -e "set terminal png size 1500,700; set output 'plot_latency.png'; set key outside; 
plot 'nginx_latency' with linespoints, 'baseline_latency' with linespoints, 'armour-allow_latency' with linespoints, 'envoy_latency' with linespoints, 'armour-http-id_latency' with linespoints, 'armour-http-req_latency' with linespoints, 'armour-log-http_latency' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'plot_throughput.png'; set key outside; 
plot 'nginx_throughput' with linespoints, 'baseline_throughput' with linespoints, 'armour-allow_throughput' with linespoints, 'envoy_throughput' with linespoints, 'armour-http-id_throughput' with linespoints, 'armour-http-req_throughput' with linespoints, 'armour-log-http_throughput' with linespoints"