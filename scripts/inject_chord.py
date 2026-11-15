#!/usr/bin/env python3
"""Inject a real AltGr+L chord (code 100 + code 38) into an evdev node.

Usage: python3 scripts/inject_chord.py /dev/input/event20
Order: AltGr down, L down, L up, AltGr up — with a SYN_REPORT after each event.
"""
import os
import struct
import sys
import time

node = sys.argv[1]
ALTGR, L = 100, 38
fd = os.open(node, os.O_WRONLY)


def ev(code: int, value: int) -> bytes:
    t = time.time()
    sec, usec = int(t), int((t % 1) * 1e6)
    return struct.pack("llHHi", sec, usec, 0x01, code, value)


def syn() -> bytes:
    t = time.time()
    sec, usec = int(t), int((t % 1) * 1e6)
    return struct.pack("llHHi", sec, usec, 0x00, 0x00, 0x00)


for code, value in [(ALTGR, 1), (L, 1), (L, 0), (ALTGR, 0)]:
    os.write(fd, ev(code, value) + syn())
    time.sleep(0.05)
os.close(fd)
print("injected AltGr+L chord into", node)
