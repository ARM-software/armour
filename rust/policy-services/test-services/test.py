#!/usr/bin/env python

from __future__ import print_function
import argparse
import socket
import random
import capnp

import test_capnp

i = 2
class TestImpl(test_capnp.External.Server):

    def call(self, name, args, **kwargs):
        global i
        if name == 'set':
            if len(args) == 1:
                i = args[0].int64
            else:
                raise Exception('set can only have one argument')
        elif name == 'inc':
            if len(args) == 0:
                i+=1
                return test_capnp.External.Value.new_message(int64=i)
            else:
                raise Exception('inc cannot have any arguments')
        elif name == 'rev':
            if len(args) == 1:
                lst = test_capnp.External.Value.new_message()
                items = lst.init('list', len(args[0].list))
                j = 0
                for item in reversed(args[0].list):
                    l = items[j]
                    j += 1
                    l.tuple = [item.tuple[1], item.tuple[0]]
                return lst
            else:
                raise Exception('rev can only have one argument')


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
