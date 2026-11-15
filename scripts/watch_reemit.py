#!/usr/bin/env python3
"""Watch the tool's uinput re-emit device and log EV_KEY events until timeout.

Usage: python3 scripts/watch_reemit.py <seconds> <outfile>
"""
import fcntl, os, struct, sys, time, glob

seconds = float(sys.argv[1])
out = sys.argv[2]
EVIOCGNAME = 0x80804506
EV_KEY = 0x01
deadline = time.time() + seconds
fd = None
while time.time() < deadline and fd is None:
    for path in sorted(glob.glob("/dev/input/event*")):
        try:
            f = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
            buf = bytearray(128)
            fcntl.ioctl(f, EVIOCGNAME, buf, True)
            name = bytes(buf).split(b"\0")[0].decode(errors="replace")
            if name == "static-dictation-kbd":
                fd = f
                print("monitor device:", path, flush=True)
                break
            os.close(f)
        except OSError:
            try:
                os.close(f)
            except (OSError, UnboundLocalError):
                pass
    if fd is None:
        time.sleep(0.05)

events = []
if fd is not None:
    while time.time() < deadline:
        try:
            data = os.read(fd, 24)
        except BlockingIOError:
            time.sleep(0.01)
            continue
        except OSError:
            break
        if len(data) < 24:
            continue
        typ, code, value = struct.unpack("HHi", data[16:24])
        if typ == EV_KEY:
            events.append((code, value))
    os.close(fd)
with open(out, "w") as fh:
    fh.write("\n".join(f"{c},{v}" for c, v in events) + ("\n" if events else ""))
print("reemit device:", "found" if fd is not None else "NOT FOUND", "events:", len(events))
