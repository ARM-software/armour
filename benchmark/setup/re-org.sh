
cd ../results/proxy/
dir=`ls -d -- */`
for path in $dir
do
cd $path
zip -r raw-data  ./* -x *.png
shopt -s extglob
rm -r !\(*.png\|*.zip\)
cd ..
done