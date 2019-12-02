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
  cp $f ../results/server/nginx/$file-$i
elif [[ $f == *"apache"* ]]; then
cp $f ../results/server/apache/$file-$i
elif [[ $f == *"cherokee"* ]]; then
cp $f ../results/server/cherokee/$file-$i
elif [[ $f == *"lighttpd"* ]]; then
cp $f ../results/server/lighttpd/$file-$i
elif [[ $f == *"srv-actix"* ]]; then
cp $f ../results/server/actix/$file-$i
elif [[ $f == *"srv-hyper"* ]]; then
cp $f ../results/server/hyper/$file-$i
fi
i=$((i+1))
done

