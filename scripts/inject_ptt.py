import fcntl, os, select, struct, time, sys

node = sys.argv[1]          # /dev/input/eventN (grabbed keyboard)
key  = int(sys.argv[2])     # 97 = RightCtrl, 56/98 = KP0 etc
hold = float(sys.argv[3])   # seconds to hold

fd = os.open(node, os.O_WRONLY)

def ev(t, v):
    # struct input_event: timeval(tv_sec, tv_usec), __u16 type, __u16 code, __s32 value
    sec, usec = int(t), int((t % 1) * 1e6)
    return struct.pack('llHHi', sec, usec, 0x01, key, v)
def syn(t):
    sec, usec = int(t), int((t % 1) * 1e6)
    return struct.pack('llHHi', sec, usec, 0x00, 0x00, 0x00)

now = time.time()
os.write(fd, ev(now, 1) + syn(now))
time.sleep(hold)
now = time.time()
os.write(fd, ev(now, 0) + syn(now))
os.close(fd)
print("injected key", key, "hold", hold)
