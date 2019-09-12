#!/usr/bin/env python

from __future__ import print_function
import argparse
import socket
import random
import capnp
import docker

import test_capnp
client = docker.from_env()

class TestImpl(test_capnp.External.Server):

    def call(self, name, args, **kwargs):
        if name == 'info':
            if len(args) == 3:
                hosts = (args[0].tuple)[0].list
                ips = (args[0].tuple)[1].list
                ports = ((args[0].tuple)[2].tuple)
                '''for host in hosts:
                    print(host.text)
                for ip in ips:
                    for n in ip.tuple:
                        print(n.int64)
                for port in ports:
                    print(port.int64)'''
                list = test_capnp.External.Value.new_message()
                items = list.init('list', len(hosts))
                j = 0
                if args[1].text == 'container':
                    attr = ['id', 'image', 'labels', 'status', 'name', 'short_id']
                    if args[2].text in attr:
                        for host in hosts:
                            container = client.containers.get(host.text)
                            l = items[j]
                            j += 1
                            l.text = str(getattr(container,args[2].text))
                        print(list)
                        return list
                    else:
                        raise Exception('can only retrieve id, image, labels and status of a container')
                elif args[1].text == 'image':
                    attr = ['id', 'tag', 'labels', 'digest']
                    if args[2].text in attr:
                        for host in hosts:
                            container = client.containers.get(host.text)
                            image = str(getattr(container,'image'))[len("<Image: '"):][:-2]
                            l = items[j]
                            j+=1
                            if args[2].text in ['digest']:
                                l.text = str(client.images.get_registry_data(image).id)
                            else:
                                l.text = str(getattr(client.images.get(image),args[2].text))
                        return list
                    else:
                        raise Exception('can only retrieve id, tag, labels and digest of an image')
            else:
                raise Exception('info can only have one argument')


def parse_args():
    parser = argparse.ArgumentParser(usage='''Runs the server bound to the\
given address/port ADDRESS may be '*' to bind to all local addresses.\
:PORT may be omitted to choose a port automatically. ''')

    parser.add_argument("address", help="ADDRESS[:PORT]")

    return parser.parse_args()


def main():
    address = parse_args().address

    server = capnp.TwoPartyServer(address, bootstrap=TestImpl())
    server.run_forever()

if __name__ == '__main__':
    main()
