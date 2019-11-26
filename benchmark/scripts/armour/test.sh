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
}' a1 > r1