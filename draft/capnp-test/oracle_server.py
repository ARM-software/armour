#!/usr/bin/env python

from __future__ import print_function
import argparse
import socket
import random
import capnp

import oracle_capnp


class OracleImpl(oracle_capnp.Oracle.Server):

    def eval(self, calls, **kwargs):
        print(calls)
        return [oracle_capnp.Oracle.Value.new_message(float64=3.141),
                oracle_capnp.Oracle.Value.new_message(text="that worked")]

    def update(self, calls, **kwargs):
        for c in calls:
            print(c)


def parse_args():
    parser = argparse.ArgumentParser(usage='''Runs the server bound to the\
    given address/port ADDRESS may be '*' to bind to all local addresses.\
    :PORT may be omitted to choose a port automatically. ''')

    parser.add_argument("address", help="ADDRESS[:PORT]")

    return parser.parse_args()


def main():
    address = parse_args().address

    server = capnp.TwoPartyServer(address, bootstrap=OracleImpl())
    server.run_forever()


if __name__ == '__main__':
    main()
