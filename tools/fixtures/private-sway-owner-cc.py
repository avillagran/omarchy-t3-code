#!/usr/bin/python3
import os,sys
args=sys.argv[1:]
if any(arg.endswith(('/sway/server.c','/sway/ipc-server.c')) for arg in args):args.append('-D_GNU_SOURCE')
os.execv('/usr/bin/cc',['cc',*args])
