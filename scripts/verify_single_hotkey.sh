#!/usr/bin/env bash
# Live proof: AltGr+L is the only hotkey; the whole chord is consumed.
set -u
cd /home/alex/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665
NODE=${1:-/dev/input/event9}
ORIG=$(pactl get-default-source)
pactl load-module module-null-sink sink_name=hkonly-sink >/dev/null 2>&1
pactl set-default-source hkonly-sink.monitor
restore() {
    pactl set-default-source "$ORIG" >/dev/null 2>&1
    pactl unload-module module-null-sink >/dev/null 2>&1
}
trap restore EXIT
rm -f /tmp/hkonly.log /tmp/hkonly.stdout /tmp/hkonly-reemit.txt /tmp/hkonly-watch.log
python3 scripts/watch_reemit.py 10 /tmp/hkonly-reemit.txt >/tmp/hkonly-watch.log 2>&1 &
WATCH=$!
cp target/release/static-dictation /tmp/static-dictation-hk
env -u WAYLAND_DISPLAY -u DISPLAY timeout 10 /tmp/static-dictation-hk \
    >/tmp/hkonly.stdout 2>/tmp/hkonly.log &
BIN=$!
sleep 2
for k in 97 82 110; do
    python3 scripts/inject_ptt.py "$NODE" "$k" 0.05 >/dev/null
    sleep 0.4
done
sleep 0.6
python3 scripts/inject_chord.py "$NODE" >/dev/null
sleep 0.6
# Hold AltGr and tap L twice: still exactly one dictation action, and no "l" typed.
python3 - "$NODE" <<'PY'
import os, struct, sys, time
fd = os.open(sys.argv[1], os.O_WRONLY)
def e(code, value):
    t = time.time()
    return struct.pack("llHHi", int(t), int((t % 1) * 1e6), 0x01, code, value)
def syn():
    t = time.time()
    return struct.pack("llHHi", int(t), int((t % 1) * 1e6), 0, 0, 0)
for code, value in [(100, 1), (38, 1), (38, 0), (38, 1), (38, 0), (100, 0),
                    (38, 1), (38, 0),                 # plain L after the chord: must be "l", not "ł"
                    (100, 1), (30, 1), (30, 0), (100, 0),  # AltGr+A: still ą
                    (38, 1), (38, 0)]:                # and one more plain L
    os.write(fd, e(code, value) + syn())
    time.sleep(0.05)
os.close(fd)
PY
sleep 1.5
wait "$WATCH" || true
wait "$BIN" || true
echo "=== stdout ==="; cat /tmp/hkonly.stdout
echo "=== hk log (hotkey lines) ==="; grep -E "armed|->|toggle event|hk:" /tmp/hkonly.log
echo "=== re-emit EV_KEY (code,value) ==="; cat /tmp/hkonly-reemit.txt
echo "=== watcher ==="; cat /tmp/hkonly-watch.log
echo "=== regression assertions ==="
python3 - <<'PY'
lines = open('/tmp/hkonly-reemit.txt').read().split()
log = open('/tmp/hkonly.log').read()
check = {
    'two toggles, one per chord': log.count('-> Toggle') == 2,
    'no L from either chord': lines.count('38,1') == 2,  # only the plain L taps
    'exactly one AltGr press (AltGr+A)': lines.count('100,1') == 1,
    'AltGr press has a matching release': lines.count('100,0') == 1,
    'level-3 char forwarded': '30,1' in lines and '30,0' in lines,
}
for name, ok in check.items():
    print(('PASS' if ok else 'FAIL'), name)
raise SystemExit(0 if all(check.values()) else 1)
PY
