 #!/bin/bash
mkdir ../results/server/
mkdir ../results/server/nginx ../results/server/apache  ../results/server/cherokee ../results/server/lighttpd ../results/server/actix ../results/server/hyper
cd ../raw-data/
loc=`find . -type f -print0 | xargs -0 ls -t`
i=1
for f in $loc
do
file=`echo $f| cut -f4-5 -d"/" | sed "s/\///"`
if [[ $f == *"srv-nginx"* ]]; then
  if [[ $f != *"_"* ]]; then
    cp $f ../results/server/nginx/$file-$i
  elif [[ $f == *"scalability_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/nginx/scalability_log
    cat $f >> ../results/server/nginx/scalability_log
  elif [[ $f == *"throughput_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/nginx/throughput_log
    cat $f >> ../results/server/nginx/throughput_log
  fi
elif [[ $f == *"apache"* ]]; then
  if [[ $f != *"_"* ]]; then
    cp $f ../results/server/apache/$file-$i
  elif [[ $f == *"scalability_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/apache/scalability_log
    cat $f >> ../results/server/apache/scalability_log
  elif [[ $f == *"throughput_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/apache/throughput_log
    cat $f >> ../results/server/apache/throughput_log
  fi
elif [[ $f == *"cherokee"* ]]; then
  if [[ $f != *"_"* ]]; then
    cp $f ../results/server/cherokee/$file-$i
  elif [[ $f == *"scalability_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/cherokee/scalability_log
    cat $f >> ../results/server/cherokee/scalability_log
  elif [[ $f == *"throughput_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/cherokee/throughput_log
    cat $f >> ../results/server/cherokee/throughput_log
  fi
elif [[ $f == *"lighttpd"* ]]; then
  if [[ $f != *"_"* ]]; then
    cp $f ../results/server/lighttpd/$file-$i
  elif [[ $f == *"scalability_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/lighttpd/scalability_log
    cat $f >> ../results/server/lighttpd/scalability_log
  elif [[ $f == *"throughput_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/lighttpd/throughput_log
    cat $f >> ../results/server/lighttpd/throughput_log
  fi
elif [[ $f == *"srv-actix"* ]]; then
  if [[ $f != *"_"* ]]; then
    cp $f ../results/server/actix/$file-$i
  elif [[ $f == *"scalability_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/actix/scalability_log
    cat $f >> ../results/server/actix/scalability_log
  elif [[ $f == *"throughput_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/actix/throughput_log
    cat $f >> ../results/server/actix/throughput_log
  fi
elif [[ $f == *"srv-hyper"* ]]; then
  if [[ $f != *"_"* ]]; then
    cp $f ../results/server/hyper/$file-$i
  elif [[ $f == *"scalability_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/hyper/scalability_log
    cat $f >> ../results/server/hyper/scalability_log
  elif [[ $f == *"throughput_log"* ]]; then
    echo $f | cut -f5-6 -d"/" >> ../results/server/hyper/throughput_log
    cat $f >> ../results/server/hyper/throughput_log
  fi
fi
i=$((i+1))
done

