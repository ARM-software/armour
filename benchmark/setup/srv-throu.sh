
cd ../results/server/
dir=`ls -d -- */`

for path in $dir
do
    for file in $path/*
    do
    if [[ $file == *"throughput"* ]]; then
        grep -e "0.990625" -e "Requests/sec:" $file | awk '{print $1 " " $2}' | awk '{printf $0 (NR%2?" ":"\n")}' | awk 'BEGIN {i=25000} {print i,"  ",$1 ,"  ",$4; i=i-500}' >> "$path"temp
    elif [[ $file == *"scalability"* ]]; then
    grep -e "Requests/sec:" $file | awk '{print $2}' | awk 'BEGIN {i=2501} {print i,"  ",$1 ; i=i-100}' >> "$path"temp-scalability
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
}' "$path"temp-scalability > ${path%/}_scalability
echo "`cat "${path%/}_scalability" | sort -k 1 -n`" >  ${path%/}_scalability
done
rm "$path"temp-throughput
rm "$path"temp-scalability

cd ../results/server
gnuplot -e "set terminal png size 1500,700; set output 'plot_throughput.png'; set key outside; set title 'Throughtput'; set xlabel 'Requests/second'; set ylabel 'Response/second';
plot 'nginx_throughput' with linespoints, 'actix_throughput' with linespoints, 'hyper_throughput' with linespoints, 'cherokee_throughput' with linespoints, 'lighttpd_throughput' with linespoints, 'apache_throughput' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'plot_latency.png'; set key outside; set title 'Latency'; set xlabel 'Requests/second'; set ylabel 'seconds';
plot 'nginx_latency' with linespoints, 'actix_latency' with linespoints, 'hyper_latency' with linespoints, 'cherokee_latency' with linespoints, 'lighttpd_latency' with linespoints, 'apache_latency' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'plot_scalability.png'; set key outside; set title 'Scalability'; set xlabel 'Concurrent connections'; set ylabel 'Response/second';
plot 'nginx_scalability' with linespoints, 'actix_scalability' with linespoints, 'hyper_scalability' with linespoints, 'cherokee_scalability' with linespoints, 'lighttpd_scalability' with linespoints, 'apache_scalability' with linespoints"