#!/usr/local/bin/fish

mkdir dockerfile-patches
for i in (/bin/ls -1 new-poc-instance/) 
    if test -e new-poc-instance/$i/Dockerfile
        diff -u new-poc-instance/$i/Dockerfile ../vagrant-poc/PoCx86/$i/Dockerfile > dockerfile-patches/$i.patch
    end
end
