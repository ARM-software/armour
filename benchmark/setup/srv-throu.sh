
cd ../results/server/
dir=`ls -d -- */`

for path in $dir
do
    for file in $path/*
    do
    if [[ $file == *"throughput-"* ]]; then
        grep -e "0.990625" -e "Requests/sec:" $file | awk '{print $1 " " $2}' | awk '{printf $0 (NR%2?" ":"\n")}' | awk 'BEGIN {i=25000} {print i,"  ",$1 ,"  ",$4; i=i-500}' >> "$path"temp
    elif [[ $file == *"scalability-"* ]]; then
        grep -e "0.990625" -e "Requests/sec:" $file | awk '{print $1 " " $2}' | awk '{printf $0 (NR%2?" ":"\n")}' | awk 'BEGIN {i=2501} {print i,"  ",$1 ,"  ",$4; i=i-100}' >> "$path"temp-scalability
    elif [[ $file == *"scalability_log" ]]; then
        i=1
        while [ $i -le 2501 ]
        do
            sed -n "/_log$i$/,/_log$((i+100))$/p" $file | awk '{print $3}' | awk /./ | sed '/[a-z]/d' | awk '{ total += $1; c++ } END { print total/c }' >> "$path"sc_log
            let "i+=100"
        done

        awk 'BEGIN {i=1} {print i,"  ",$1; i=i+100}' "$path"sc_log > ${path%/}_scalability_mem
        rm "$path"sc_log

        i=1
        while [ $i -le 2501 ]
        do
            sed -n "/_log$i$/,/_log$((i+100))$/p" $file | awk '{print $2}' | awk /./ | sed '/[a-z]/d' | awk '{ total += $1; c++ } END { print total/c }' >> "$path"sc_log
            let "i+=100"
        done

        awk 'BEGIN {i=1} {print i,"  ",$1; i=i+100}' "$path"sc_log > ${path%/}_scalability_cpu
        rm "$path"sc_log
    elif [[ $file == *"throughput_log" ]]; then
        i=1000
        while [ $i -le 25000 ]
        do
            sed -n "/_log$i$/,/_log$((i+1000))$/p" $file | awk '{print $3}' | awk /./ | sed '/[a-z]/d' | awk '{ total += $1; c++ } END { print total/c }'  >> "$path"th_log
            let "i+=1000"
        done
        awk 'BEGIN {i=1} {print i,"  ",$1; i=i+1000}' "$path"th_log > ${path%/}_throughput_mem
        rm "$path"th_log 

        i=1000
        while [ $i -le 25000 ]
        do
            sed -n "/_log$i$/,/_log$((i+1000))$/p" $file | awk '{print $2}' | awk /./ | sed '/[a-z]/d' | awk '{ total += $1; c++ } END { print total/c }'  >> "$path"th_log
            let "i+=1000"
        done
        awk 'BEGIN {i=1} {print i,"  ",$1; i=i+1000}' "$path"th_log > ${path%/}_throughput_cpu
        rm "$path"th_log
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
rm "$path"temp
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
}' "$path"temp-scalability > ${path%/}_lat-cnx
echo "`cat "${path%/}_lat-cnx" | sort -k 1 -n`" >  ${path%/}_lat-cnx

awk '{
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
rm "$path"temp-scalability
echo "`cat "${path%/}_scalability" | sort -k 1 -n`" >  ${path%/}_scalability

done
gnuplot -e "set terminal png size 1500,700; set output 'plot_throughput_memory.png'; set key outside; set title 'Throughtput / memory usage'; set xlabel 'Requests/second'; set ylabel 'Real memory MB';
plot 'nginx_throughput_mem' with linespoints, 'actix_throughput_mem' with linespoints, 'hyper_throughput_mem' with linespoints, 'lighttpd_throughput_mem' with linespoints, 'apache_throughput_mem' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'plot_scalability_memory.png'; set key outside; set title 'Scalability / memory usage'; set xlabel 'Concurrent connections'; set ylabel 'Real memory MB';
plot 'nginx_scalability_mem' with linespoints, 'actix_scalability_mem' with linespoints, 'hyper_scalability_mem' with linespoints, 'lighttpd_scalability_mem' with linespoints, 'apache_scalability_mem' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'plot_throughput_cpu.png'; set key outside; set title 'Throughtput / CPU usage'; set xlabel 'Requests/second'; set ylabel 'CPU %';
plot 'nginx_throughput_cpu' with linespoints, 'actix_throughput_cpu' with linespoints, 'hyper_throughput_cpu' with linespoints, 'lighttpd_throughput_cpu' with linespoints, 'apache_throughput_cpu' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'plot_scalability_cpu.png'; set key outside; set title 'Scalability / CPU usage'; set xlabel 'Concurrent connections'; set ylabel 'CPU %';
plot 'nginx_scalability_cpu' with linespoints, 'actix_scalability_cpu' with linespoints, 'hyper_scalability_cpu' with linespoints, 'lighttpd_scalability_cpu' with linespoints, 'apache_scalability_cpu' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'plot_throughput.png'; set key outside; set title 'Throughtput'; set xlabel 'Requests/second'; set ylabel 'Response/second';
plot 'nginx_throughput' with linespoints, 'actix_throughput' with linespoints, 'hyper_throughput' with linespoints, 'cherokee_throughput' with linespoints, 'lighttpd_throughput' with linespoints, 'apache_throughput' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'plot_latency.png'; set key outside; set title 'Latency'; set xlabel 'Requests/second'; set ylabel 'milliseconds';
plot 'nginx_latency' with linespoints, 'actix_latency' with linespoints, 'hyper_latency' with linespoints, 'cherokee_latency' with linespoints, 'lighttpd_latency' with linespoints, 'apache_latency' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'plot_scalability.png'; set key outside; set title 'Scalability'; set xlabel 'Concurrent connections'; set ylabel 'Response/second';
plot 'nginx_scalability' with linespoints, 'actix_scalability' with linespoints, 'hyper_scalability' with linespoints, 'cherokee_scalability' with linespoints, 'lighttpd_scalability' with linespoints, 'apache_scalability' with linespoints"

gnuplot -e "set terminal png size 1500,700; set output 'plot_lat-conx.png'; set key outside; set title 'Latency / concurrent connections'; set xlabel 'Concurrent connections'; set ylabel 'milliseconds';
plot 'nginx_lat-cnx' with linespoints, 'actix_lat-cnx' with linespoints, 'hyper_lat-cnx' with linespoints, 'cherokee_lat-cnx' with linespoints, 'lighttpd_lat-cnx' with linespoints, 'apache_lat-cnx' with linespoints"

rm nginx_throughput_mem actix_throughput_mem hyper_throughput_mem lighttpd_throughput_mem apache_throughput_mem
rm nginx_throughput_cpu actix_throughput_cpu hyper_throughput_cpu lighttpd_throughput_cpu apache_throughput_cpu
rm nginx_scalability_mem actix_scalability_mem hyper_scalability_mem lighttpd_scalability_mem apache_scalability_mem
rm nginx_scalability_cpu actix_scalability_cpu hyper_scalability_cpu lighttpd_scalability_cpu apache_scalability_cpu
rm nginx_throughput actix_throughput hyper_throughput lighttpd_throughput apache_throughput cherokee_throughput
rm nginx_scalability actix_scalability hyper_scalability lighttpd_scalability apache_scalability cherokee_scalability
rm nginx_latency actix_latency hyper_latency cherokee_latency lighttpd_latency apache_latency 
rm nginx_lat-cnx actix_lat-cnx hyper_lat-cnx cherokee_lat-cnx lighttpd_lat-cnx apache_lat-cnx 
