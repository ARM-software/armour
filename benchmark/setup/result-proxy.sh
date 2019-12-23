#!/bin/bash
mkdir ../results/new/
mkdir ../results/new/linkerd ../results/new/armour-all-async-log ../results/new/armour-all-log ../results/new/armour-all ../results/new/armour-async-log ../results/new/envoy ../results/new/nginx ../results/new/baseline ../results/new/armour-log ../results/new/armour-req-log ../results/new/armour-req-method ../results/new/armour-req-res ../results/new/armour-req  ../results/new/armour-res  ../results/new/armour-srv-payload  ../results/new/armour-allow
cd ../raw-data/
loc=`find . -type f -print0 | xargs -0 ls -t`
i=1
for f in $loc
do
file=`echo $f| cut -f4-5 -d"/" | sed "s/\///"`

    if [[ $f == *"armour-allow"* ]]; then
        cp $f ../results/new/armour-allow/$file-$i
    elif [[ $f == *"nginx"* ]]; then
        cp $f ../results/new/nginx/$file-$i
    elif [[ $f == *"linkerd"* ]]; then
        cp $f ../results/new/linkerd/$file-$i
    elif [[ $f == *"envoy"* ]]; then
        cp $f ../results/new/envoy/$file-$i
    elif [[ $f == *"baseline"* ]]; then
        cp $f ../results/new/baseline/$file-$i
    elif [[ $f == *"armour-all-async-log"* ]]; then
        cp $f ../results/new/armour-all-async-log/$file-$i
    elif [[ $f == *"armour-all-log"* ]]; then
        cp $f ../results/new/armour-all-log/$file-$i
    elif [[ $f == *"armour-allow"* ]]; then
        cp $f ../results/new/armour-allow/$file-$i
    elif [[ $f == *"armour-all"* ]]; then
        cp $f ../results/new/armour-all/$file-$i
    elif [[ $f == *"armour-req-res"* ]]; then
        cp $f ../results/new/armour-req-res/$file-$i
    elif [[ $f == *"armour-req-method"* ]]; then
        cp $f ../results/new/armour-req-method/$file-$i
    elif [[ $f == *"armour-req-log"* ]]; then
        cp $f ../results/new/armour-req-log/$file-$i
    elif [[ $f == *"armour-req"* ]]; then
        cp $f ../results/new/armour-req/$file-$i
    elif [[ $f == *"armour-res"* ]]; then
        cp $f ../results/new/armour-res/$file-$i
    elif [[ $f == *"armour-srv-payload"* ]]; then
        cp $f ../results/new/armour-srv-payload/$file-$i
    elif [[ $f == *"armour-log"* ]]; then
        cp $f ../results/new/armour-log/$file-$i
    elif [[ $f == *"armour-async-log"* ]]; then
        cp $f ../results/new/armour-async-log/$file-$i
    fi

i=$((i+1))
done

cd ../results/new/
dir=`ls -d -- */`
for path in $dir
do
    for file in $path/*
        do
        if [[ $file == *"latency"* && $file != *"_log" ]]; then
            grep -e "0.990625" -e "Requests/sec:" $file | awk '{print $1 " " $2}' | awk '{printf $0 (NR%2?" ":"\n")}'| awk 'BEGIN {i=25000} {print i,"  ",$1 ,"  ",$4; i=i-1000}' >> "$path"temp
        elif [[ $file == *"Scalability"* && $file != *"_log" ]]; then
            grep -e "0.990625" -e "Requests/sec:" $file | awk '{print $1 " " $2}' | awk '{printf $0 (NR%2?" ":"\n")}' | awk 'BEGIN {i=2501} {print i,"  ",$1 ,"  ",$4; i=i-100}' >> "$path"temp-scalability
    
fi
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
    div = 1
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
    div = 0.4
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
    div = 0.4
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
}' "$path"temp-scalability > ${path%/}_scalability
echo "`cat "${path%/}_scalability" | sort -k 1 -n`" >  ${path%/}_scalability
rm "$path"temp-scalability
done

gnuplot -e "set terminal png size 1500,700; set output 'all_latency.png'; set key outside; set title 'Latency'; set xlabel 'Requests/second'; set ylabel 'Milliseconds';
plot 'linkerd_latency' with linespoints title 'linkerd', 'nginx_latency' with linespoints title 'nginx', 'baseline_latency' with linespoints title 'baseline', 'armour-all-async-log_latency' with linespoints title 'armour all-async-log', 'envoy_latency' with linespoints title 'envoy', 'armour-all-log_latency' title 'armour all-log' with linespoints, 'armour-all_latency' with linespoints title 'armour all', 'armour-async-log_latency' title 'armour async-log' with linespoints, 'armour-log_latency' with linespoints title 'armour log', 'armour-req-log_latency' title 'armour req-log' with linespoints, 'armour-req-method_latency' title 'armour req-method' with linespoints, 'armour-req-res_latency' title 'armour req-res' with linespoints, 'armour-req_latency' title 'armour req' with linespoints, 'armour-res_latency' title 'armour res' with linespoints, 'armour-srv-payload_latency' title 'armour srv-payload' with linespoints, 'armour-allow_latency' title 'armour allow' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'all_throughput.png'; set key outside; set title 'Throughput'; set xlabel 'Requests/second'; set ylabel 'Response/second';
plot 'linkerd_throughput' with linespoints title 'linkerd', 'nginx_throughput' with linespoints title 'nginx',  'baseline_throughput' with linespoints title 'baseline', 'armour-all-async-log_throughput' with linespoints title 'armour all-async-log', 'envoy_throughput' title 'envoy' with linespoints, 'armour-all-log_throughput' title 'armour all-log' with linespoints, 'armour-all_throughput' with linespoints title 'armour all', 'armour-async-log_throughput' title 'armour async-log' with linespoints, 'armour-log_throughput' title 'armour log' with linespoints, 'armour-req-log_throughput' title 'armour req-log' with linespoints, 'armour-req-method_throughput' title 'armour req-method' with linespoints, 'armour-req-res_throughput' title 'armour req-res'  with linespoints, 'armour-req_throughput' title 'armour req'  with linespoints, 'armour-res_throughput' title 'armour res' with linespoints, 'armour-srv-payload_throughput' title 'armour srv-payload' with linespoints, 'armour-allow_throughput' title 'armour allow' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'armour_latency.png'; set key outside; set title 'Armour latency'; set xlabel 'Requests/second'; set ylabel 'Milliseconds';
plot  'armour-all-async-log_latency' with linespoints title ' all-async-log', 'armour-all-log_latency' title ' all-log' with linespoints, 'armour-all_latency' with linespoints title ' all', 'armour-async-log_latency' with linespoints title ' async-log', 'armour-log_latency' title ' log' with linespoints, 'armour-req-log_latency' title ' req-log' with linespoints, 'armour-req-method_latency' title ' req-method' with linespoints, 'armour-req-res_latency' title ' req-res' with linespoints, 'armour-req_latency' title ' req' with linespoints, 'armour-res_latency' title ' res' with linespoints, 'armour-srv-payload_latency' title ' srv-payload' with linespoints, 'armour-allow_latency' title ' allow' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'armour_throughput.png'; set key outside; set title 'Armour throughput'; set xlabel 'Requests/second'; set ylabel 'Response/second';
plot 'armour-all-async-log_throughput' with linespoints title ' all-async-log', 'armour-all-log_throughput' title ' all-log' with linespoints, 'armour-all_throughput' title ' all' with linespoints, 'armour-async-log_throughput' title ' async-log' with linespoints, 'armour-log_throughput' title ' log' with linespoints, 'armour-req-log_throughput' title ' req-log' with linespoints, 'armour-req-method_throughput'  title ' req-method' with linespoints, 'armour-req-res_throughput' title ' req-res' with linespoints, 'armour-req_throughput' title ' req' with linespoints, 'armour-res_throughput'  title ' res' with linespoints, 'armour-srv-payload_throughput'  title ' srv-payload' with linespoints, 'armour-allow_throughput' title ' allow' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'armour-policy_latency.png'; set key outside; set title 'Armour policies latency'; set xlabel 'Requests/second'; set ylabel 'Milliseconds';
plot   'armour-all_latency' with linespoints title ' all',  'armour-req-method_latency' with linespoints title 'req-method', 'armour-req-res_latency' with linespoints title 'req-res', 'armour-req_latency' with linespoints title 'req', 'armour-res_latency' with linespoints title 'res', 'armour-srv-payload_latency' with linespoints title 'srv-payload', 'armour-allow_latency' with linespoints title 'allow'"

gnuplot -e "set terminal png size 1500,700; set output 'armour-policy_throughput.png'; set key outside; set title 'Armour policies latency'; set xlabel 'Requests/second'; set ylabel 'Response/second';
plot  'armour-all_throughput' with linespoints title ' all', 'armour-req-method_throughput' with linespoints title 'req-method', 'armour-req-res_throughput' with linespoints title 'req-res', 'armour-req_throughput' with linespoints title 'req', 'armour-res_throughput' with linespoints title 'res', 'armour-srv-payload_throughput' with linespoints title 'srv-payload', 'armour-allow_throughput' with linespoints title 'allow'"

gnuplot -e "set terminal png size 1500,700; set output 'armour-log_latency.png'; set key outside; set title 'Armour oracle latency'; set xlabel 'Requests/second'; set ylabel 'Milliseconds';
plot  'armour-all-async-log_latency' with linespoints title 'all-async-log', 'armour-all-log_latency' with linespoints title 'all-log',  'armour-async-log_latency' with linespoints title 'async-log', 'armour-log_latency' with linespoints title 'log', 'armour-req-log_latency' with linespoints title 'req-log',  'armour-allow_latency' with linespoints title 'allow'"

gnuplot -e "set terminal png size 1500,700; set output 'armour-log_throughput.png'; set key outside; set title 'Armour oracle throughput'; set xlabel 'Requests/second'; set ylabel 'Response/second';
plot 'armour-all-async-log_throughput' with linespoints title 'all-async-log', 'armour-all-log_throughput' with linespoints title 'all-log', 'armour-async-log_throughput' with linespoints title 'async-log', 'armour-log_throughput' with linespoints title 'log', 'armour-req-log_throughput' with linespoints title 'req-log', 'armour-allow_throughput' with linespoints title 'allow'"

gnuplot -e "set terminal png size 1500,700; set output 'plot_scalability.png'; set key outside; set title 'Scalability'; set xlabel 'Concurrent connections'; set ylabel 'Response/second';
plot 'linkerd_scalability' with linespoints title 'linkerd',  'baseline_scalability' with linespoints title 'baseline', 'envoy_scalability' with linespoints title 'envoy',  'nginx_scalability' with linespoints title 'nginx',  'armour-all_scalability' with linespoints title 'armour all',  'armour-allow_scalability' with linespoints title 'armour allow'"

gnuplot -e "set terminal png size 1500,700; set output 'basic_latency.png'; set key outside; set title 'Latency'; set xlabel 'Requests/second'; set ylabel 'Milliseconds';
plot 'linkerd_latency' with linespoints title 'linkerd', 'nginx_latency' with linespoints title 'nginx', 'baseline_latency' with linespoints title 'baseline', 'envoy_latency' with linespoints title 'envoy', 'armour-all_latency' with linespoints title 'armour all', 'armour-allow_latency' with linespoints title 'armour allow'"

gnuplot -e "set terminal png size 1500,700; set output 'basic_throughput.png'; set key outside; set title 'Throughput'; set xlabel 'Requests/second'; set ylabel 'Response/second';
plot 'linkerd_throughput' with linespoints title 'linkerd', 'nginx_throughput' with linespoints title 'nginx',  'baseline_throughput' with linespoints title 'baseline',  'envoy_throughput' with linespoints title 'envoy', 'armour-all_throughput' with linespoints title 'armour all',  'armour-allow_throughput' with linespoints title 'armour allow'"

gnuplot -e "set terminal png size 1500,700; set output 'memory.png'; set key outside; set title 'Memory usage'; set xlabel 'time (seconds)'; set ylabel 'Real memory MB';
plot 'armour-allow/armour-allowlatency-armour_log-8' every ::0::2800  using 1:3 with lines title 'armour allow', 'envoy/envoylatency-envoy_log-47' every ::0::2800 using 1:3 with lines title 'envoy', 'nginx/nginxlatency-nginx_log-50' every ::0::2800 using 1:3 with lines title 'nginx', 'linkerd/linkerdlatency-linkerd_log-44' every ::0::2800 using 1:3 with lines title 'linkerd'"

gnuplot -e "set terminal png size 1500,700; set output 'armour_memory.png'; set key outside; set title 'Armour memory usage'; set xlabel 'time (seconds)'; set ylabel 'Real memory MB';
plot 'armour-allow/armour-allowlatency-armour_log-8' every ::0::2800  using 1:3 with lines title 'armour allow', 'armour-all/armour-alllatency-armour_log-35'  every ::0::2800  using 1:3 with lines title 'armour all' , 'armour-all-log/armour-all-loglatency-armour_log-38'  every ::0::2800  using 1:3 with lines title 'armour all log'"

gnuplot -e "set terminal png size 1500,700; set output 'cpu.png'; set key outside; set title 'CPU usage'; set xlabel 'time (seconds)'; set ylabel 'CPU %';
plot 'armour-allow/armour-allowlatency-armour_log-8' every ::0::2800  using 1:2 with lines title 'armour allow', 'envoy/envoylatency-envoy_log-47' every ::0::2800 using 1:2 with lines title 'envoy', 'nginx/nginxlatency-nginx_log-50' every ::0::2800 using 1:2 with lines title 'nginx', 'linkerd/linkerdlatency-linkerd_log-44' every ::0::2800 using 1:2 with lines title 'linkerd'"

gnuplot -e "set terminal png size 1500,700; set output 'armour_cpu.png'; set key outside; set title 'Armour CPU usage'; set xlabel 'time (seconds)'; set ylabel 'CPU %';
plot 'armour-allow/armour-allowlatency-armour_log-8' every ::0::2800  using 1:2 with lines title 'armour allow', 'armour-all/armour-alllatency-armour_log-35'  every ::0::2800  using 1:2 with lines title 'armour all' , 'armour-all-log/armour-all-loglatency-armour_log-38'  every ::0::2800  using 1:2 with lines title 'armour all log'"
rm linkerd_latency baseline_latency armour-all-async-log_latency envoy_latency armour-all-log_latency armour-all_latency armour-async-log_latency armour-log_latency armour-req-log_latency armour-req-method_latency armour-req-res_latency armour-req_latency armour-res_latency armour-srv-payload_latency armour-allow_latency nginx_latency
rm linkerd_throughput baseline_throughput armour-all-async-log_throughput envoy_throughput armour-all-log_throughput armour-all_throughput armour-async-log_throughput armour-log_throughput armour-req-log_throughput armour-req-method_throughput armour-req-res_throughput armour-req_throughput armour-res_throughput armour-srv-payload_throughput armour-allow_throughput  nginx_throughput
rm linkerd_scalability baseline_scalability armour-all-async-log_scalability envoy_scalability armour-all-log_scalability armour-all_scalability armour-async-log_scalability armour-log_scalability armour-req-log_scalability armour-req-method_scalability armour-req-res_scalability armour-req_scalability armour-res_scalability armour-srv-payload_scalability armour-allow_scalability nginx_scalability
